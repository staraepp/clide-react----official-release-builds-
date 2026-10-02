import { useEffect, useRef, type RefObject } from "react";
import { cn } from "@/lib/cn";

/**
 * The live microphone waveform.
 *
 * Bars scroll right to left from real RMS levels, so what the user sees is
 * their own voice rather than a decorative animation. Levels are mapped on a
 * decibel scale, so ordinary speech moves the bars a long way and a whisper
 * still registers. Drawn on a canvas and
 * driven by a ref rather than React state: at 30 updates a second, re-rendering
 * a component tree per sample would cost more than the audio pipeline does.
 */

interface Props {
  /**
   * Live microphone level, 0..1, delivered as a ref rather than a prop value.
   * Levels arrive 30 times a second; a ref keeps that out of React's render
   * path entirely.
   */
  levelRef: RefObject<number>;
  /** Stops the scroll and settles the bars — used by the Done state. */
  frozen?: boolean;
  bars?: number;
  className?: string;
  color?: string;
}

export function Waveform({
  levelRef,
  frozen = false,
  bars = 9,
  className,
  color = "#ffffff",
}: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const historyRef = useRef<number[]>(Array(bars).fill(0));
  const frozenRef = useRef(frozen);

  frozenRef.current = frozen;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const context = canvas.getContext("2d");
    if (!context) return;

    let frame = 0;
    let lastPush = 0;

    const render = (now: number) => {
      frame = requestAnimationFrame(render);

      const ratio = Math.min(window.devicePixelRatio || 1, 2);
      const width = canvas.clientWidth * ratio;
      const height = canvas.clientHeight * ratio;
      if (canvas.width !== width || canvas.height !== height) {
        canvas.width = width;
        canvas.height = height;
      }

      // Advance the history at a fixed rate so the scroll speed does not
      // depend on the display's refresh rate.
      if (!frozenRef.current && now - lastPush > 34) {
        lastPush = now;
        const history = historyRef.current;
        history.push(levelRef.current ?? 0);
        if (history.length > bars) history.shift();
      }

      context.clearRect(0, 0, width, height);

      const history = historyRef.current;
      const slot = width / bars;
      // Wide, fully rounded bars: at rest they read as dots, not slivers.
      const barWidth = Math.max(2 * ratio, slot * 0.58);
      const radius = barWidth / 2;
      const middle = height / 2;

      for (let i = 0; i < history.length; i++) {
        const amplitude = decibelAmplitude(history[i]);
        const barHeight = Math.max(barWidth, amplitude * height);
        const x = i * slot + (slot - barWidth) / 2;

        // Older samples fade out toward the left.
        context.globalAlpha = 0.45 + 0.55 * (i / bars);
        context.fillStyle = color;
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
 * Map an RMS level to 0..1 on a decibel scale. -52 dB (a quiet room) is the
 * floor and -12 dB (loud speech) the ceiling, so normal speech fills most of
 * the bar instead of barely lifting it.
 */
function decibelAmplitude(level: number): number {
  if (level <= 0.0004) return 0;
  const decibels = 20 * Math.log10(level);
  return Math.min(1, Math.max(0, (decibels + 52) / 40));
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
