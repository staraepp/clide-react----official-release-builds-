import { cn } from "@/lib/cn";

const FIELD =
  "no-drag w-full rounded-xl border border-line bg-sunken px-3 text-[13px] text-ink " +
  "placeholder:text-ink-3 outline-none transition-colors focus:border-voice " +
  "disabled:cursor-not-allowed disabled:opacity-50";

export function TextField({
  className,
  ...props
}: React.InputHTMLAttributes<HTMLInputElement>) {
  return <input {...props} className={cn(FIELD, "h-10", className)} />;
}

export function TextArea({
  className,
  ...props
}: React.TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return (
    <textarea {...props} className={cn(FIELD, "py-2.5 leading-relaxed", className)} />
  );
}
