import { useEffect, useRef } from "react";
import { FRAGMENT_SHADER, VERTEX_SHADER } from "./shader.glsl";
import type { VisualIntensity } from "@/lib/types";
import { cn } from "@/lib/cn";

/**
 * The ambient shader layer.
 *
 * A utility that sits behind a browser must not burn GPU while doing nothing,
 * so this canvas:
 *
 * - draws a single static frame at `reduced` and stops,
 * - caps its frame rate per intensity rather than free-running at the display
 *   refresh rate,
 * - stops entirely when the window is hidden or unfocused,
 * - and honours the system Reduce Motion setting over the app's own setting.
 */

/** Frames per second per intensity. `reduced` renders once and stops. */
const FRAME_RATE: Record<VisualIntensity, number> = {
  reduced: 0,
  normal: 30,
  high: 60,
};

const INTENSITY_VALUE: Record<VisualIntensity, number> = {
  reduced: 0,
  normal: 0.5,
  high: 1,
};

/** Rendering at full Retina density triples the fill cost for a blurred
 *  gradient nobody inspects at pixel level. */
const MAX_PIXEL_RATIO = 1.5;

interface Props {
  intensity: VisualIntensity;
  /** Microphone level, 0..1. Only reacted to at High intensity. */
  energy?: number;
  /** True while clide is handling speech. The wash gathers; otherwise it rests. */
  active?: boolean;
  className?: string;
}

export function ShaderBackground({
  intensity,
  energy = 0,
  active = false,
  className,
}: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  // Read inside the render loop so a change does not restart WebGL.
  const energyRef = useRef(energy);
  const targetRef = useRef(0);
  const presenceRef = useRef(0);
  energyRef.current = intensity === "high" ? energy : 0;
  targetRef.current = active ? 1 : 0;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const gl = canvas.getContext("webgl", {
      alpha: false,
      antialias: false,
      depth: false,
      stencil: false,
      powerPreference: "low-power",
      preserveDrawingBuffer: false,
    });

    if (!gl) {
      // No WebGL: the CSS gradient underneath is a complete fallback.
      return;
    }

    const program = createProgram(gl);
    if (!program) return;

    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    // One oversized triangle covers the viewport with fewer vertices than a quad.
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([-1, -1, 3, -1, -1, 3]),
      gl.STATIC_DRAW,
    );

    const position = gl.getAttribLocation(program, "a_position");
    gl.enableVertexAttribArray(position);
    gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);

    const uniforms = {
      resolution: gl.getUniformLocation(program, "u_resolution"),
      time: gl.getUniformLocation(program, "u_time"),
      intensity: gl.getUniformLocation(program, "u_intensity"),
      presence: gl.getUniformLocation(program, "u_presence"),
      energy: gl.getUniformLocation(program, "u_energy"),
    };
    gl.useProgram(program);

    const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
    const resize = () => {
      const ratio = Math.min(window.devicePixelRatio || 1, MAX_PIXEL_RATIO);
      const width = Math.max(1, Math.floor(canvas.clientWidth * ratio));
      const height = Math.max(1, Math.floor(canvas.clientHeight * ratio));
      if (canvas.width === width && canvas.height === height) return false;
      canvas.width = width;
      canvas.height = height;
      gl.viewport(0, 0, width, height);
      return true;
    };

    let frame = 0;
    let lastDraw = 0;
    let elapsed = 0;
    let lastTick = performance.now();
    let running = true;

    const draw = (time: number) => {
      gl.uniform2f(uniforms.resolution, canvas.width, canvas.height);
      gl.uniform1f(uniforms.time, time);
      gl.uniform1f(
        uniforms.intensity,
        INTENSITY_VALUE[reduceMotion.matches ? "reduced" : intensity],
      );
      gl.uniform1f(uniforms.presence, presenceRef.current);
      gl.uniform1f(uniforms.energy, energyRef.current);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    };

    // Reduce Motion overrides the app's own setting rather than sitting
    // alongside it: the system preference wins.
    const targetFps = reduceMotion.matches ? 0 : FRAME_RATE[intensity];

    if (targetFps === 0) {
      resize();
      presenceRef.current = targetRef.current;
      draw(0);

      const redraw = () => {
        presenceRef.current = targetRef.current;
        if (resize()) draw(0);
      };
      window.addEventListener("resize", redraw);
      return () => {
        window.removeEventListener("resize", redraw);
        gl.deleteProgram(program);
        gl.deleteBuffer(buffer);
      };
    }

    const minimumInterval = 1000 / targetFps;

    const loop = (now: number) => {
      if (!running) return;
      frame = requestAnimationFrame(loop);

      // The clock only advances while the shader is actually animating, so a
      // window left in the background does not "catch up" when it returns.
      const delta = now - lastTick;
      lastTick = now;

      if (now - lastDraw < minimumInterval) return;
      lastDraw = now;
      elapsed += delta / 1000;

      // Ease presence rather than snapping it: the wash should gather and
      // recede, not blink on the moment recording starts.
      presenceRef.current += (targetRef.current - presenceRef.current) * 0.06;

      resize();
      draw(elapsed);
    };

    const pause = () => {
      running = false;
      cancelAnimationFrame(frame);
    };

    const resume = () => {
      if (running) return;
      running = true;
      lastTick = performance.now();
      lastDraw = 0;
      frame = requestAnimationFrame(loop);
    };

    // An unfocused or hidden window renders nothing at all.
    const onVisibility = () => (document.hidden ? pause() : resume());
    document.addEventListener("visibilitychange", onVisibility);
    window.addEventListener("blur", pause);
    window.addEventListener("focus", resume);

    resize();
    frame = requestAnimationFrame(loop);

    return () => {
      pause();
      document.removeEventListener("visibilitychange", onVisibility);
      window.removeEventListener("blur", pause);
      window.removeEventListener("focus", resume);
      gl.deleteProgram(program);
      gl.deleteBuffer(buffer);
    };
  }, [intensity]);

  return (
    <canvas
      ref={canvasRef}
      aria-hidden
      className={cn(
        "pointer-events-none absolute inset-0 h-full w-full dark:invert",
        className,
      )}
      // Painted before WebGL initialises, and the whole background if a
      // machine has no WebGL at all.
      style={{
        // A fixed light grey: in dark mode the canvas is inverted wholesale.
        background: "#f6f6f5",
      }}
    />
  );
}

function compile(
  gl: WebGLRenderingContext,
  type: number,
  source: string,
): WebGLShader | null {
  const shader = gl.createShader(type);
  if (!shader) return null;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    console.warn("shader failed to compile", gl.getShaderInfoLog(shader));
    gl.deleteShader(shader);
    return null;
  }
  return shader;
}

function createProgram(gl: WebGLRenderingContext): WebGLProgram | null {
  const vertex = compile(gl, gl.VERTEX_SHADER, VERTEX_SHADER);
  const fragment = compile(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER);
  if (!vertex || !fragment) return null;

  const program = gl.createProgram();
  if (!program) return null;
  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.linkProgram(program);

  // The shaders are owned by the program once linked.
  gl.deleteShader(vertex);
  gl.deleteShader(fragment);

  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    console.warn("shader program failed to link", gl.getProgramInfoLog(program));
    return null;
  }
  return program;
}
