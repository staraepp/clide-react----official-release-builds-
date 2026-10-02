import { useEffect, useRef, type RefObject } from "react";
import { cn } from "@/lib/cn";

/**
 * The live microphone waveform.
 *
 * A row of round bars that stay where they are. Each one swells quickly when
 * the voice arrives and relaxes slowly once it stops, so speech reads as
 * breathing rather than flicker. The centre reacts first and the outer bars
 * follow a beat later, which makes every swell travel outward and settle back.
 *
 * Levels are mapped on a decibel scale with a soft curve, so a whisper still
 * registers but ordinary speech does not slam every bar to full height. Drawn
 * on a canvas and driven by a ref rather than React state: re-rendering a
 * component tree 30 times a second would cost more than the audio pipeline.
 */

interface Props {
  /**
   * Live microphone level, 0..1, delivered as a ref rather than a prop value.
   * Levels arrive 30 times a second; a ref keeps that out of React's render
   * path entirely.
   */
  levelRef: RefObject<number>;
  /** Lets the bars relax to rest — used by the Done state. */
  frozen?: boolean;
  bars?: number;
  className?: string;
  color?: string;
}

/** Seconds for a bar to close most of the gap when it should grow / shrink. */
const ATTACK = 0.07;
const RELEASE = 0.32;
/** How long the outer bars lag the centre, per step away from it. */
const RIPPLE_STEP_MS = 55;
const SAMPLE_INTERVAL_MS = 34;
/** The outermost bars never reach the height of the centre ones. */
const EDGE_REACH = 0.55;

export function Waveform({
  levelRef,
  frozen = false,
  bars = 9,
  className,
  color = "#ffffff",
}: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const frozenRef = useRef(frozen);

  frozenRef.current = frozen;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const context = canvas.getContext("2d");
    if (!context) return;

    const centre = (bars - 1) / 2;
    const lag = Array.from({ length: bars }, (_, i) =>
      Math.round((Math.abs(i - centre) * RIPPLE_STEP_MS) / SAMPLE_INTERVAL_MS),
    );
    const reach = Array.from(
      { length: bars },
      (_, i) => EDGE_REACH + (1 - EDGE_REACH) * Math.cos((Math.abs(i - centre) / (centre || 1)) * (Math.PI / 2)),
    );
    const history: number[] = Array(Math.max(...lag) + 1).fill(0);
    const heights: number[] = Array(bars).fill(0);

    let frame = 0;
    let lastPush = 0;
    let lastFrame = 0;

    const render = (now: number) => {
      frame = requestAnimationFrame(render);
      const dt = lastFrame ? Math.min(0.1, (now - lastFrame) / 1000) : 0.016;
      lastFrame = now;

      const ratio = Math.min(window.devicePixelRatio || 1, 2);
      const width = canvas.clientWidth * ratio;
      const height = canvas.clientHeight * ratio;
      if (canvas.width !== width || canvas.height !== height) {
        canvas.width = width;
        canvas.height = height;
      }

      if (now - lastPush > SAMPLE_INTERVAL_MS) {
        lastPush = now;
        history.push(frozenRef.current ? 0 : decibelAmplitude(levelRef.current ?? 0));
        history.shift();
      }

      context.clearRect(0, 0, width, height);

      const slot = width / bars;
      const barWidth = Math.max(2 * ratio, slot * 0.58);
      const radius = barWidth / 2;
      const middle = height / 2;
      context.fillStyle = color;
      context.globalAlpha = 0.95;

      for (let i = 0; i < bars; i++) {
        const target = history[history.length - 1 - lag[i]] * reach[i];
        const time = target > heights[i] ? ATTACK : RELEASE;
        heights[i] += (target - heights[i]) * (1 - Math.exp(-dt / time));

        const barHeight = Math.max(barWidth, heights[i] * height);
        const x = i * slot + (slot - barWidth) / 2;
        roundedBar(context, x, middle - barHeight / 2, barWidth, barHeight, radius);
      }
      context.globalAlpha = 1;
    };

    frame = requestAnimationFrame(render);
    return () => cancelAnimationFrame(frame);
  }, [bars, color, levelRef]);

  return (
    <canvas
      ref={canvasRef}
      aria-hidden
      className={cn("h-full w-full", className)}
    />
  );
}

/**
 * Map an RMS level to 0..1 on a decibel scale, then ease the curve. -52 dB (a
 * quiet room) is the floor and -8 dB (loud speech) the ceiling; the exponent
 * keeps ordinary speech in the middle of the range instead of pinned at the top.
 */
function decibelAmplitude(level: number): number {
  if (level <= 0.0004) return 0;
  const decibels = 20 * Math.log10(level);
  const linear = Math.min(1, Math.max(0, (decibels + 52) / 44));
  return Math.pow(linear, 1.35);
}

function roundedBar(
  context: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number,
  radius: number,
) {
  const r = Math.min(radius, width / 2, height / 2);
  context.beginPath();
  context.moveTo(x + r, y);
  context.arcTo(x + width, y, x + width, y + height, r);
  context.arcTo(x + width, y + height, x, y + height, r);
  context.arcTo(x, y + height, x, y, r);
  context.arcTo(x, y, x + width, y, r);
  context.closePath();
  context.fill();
}
