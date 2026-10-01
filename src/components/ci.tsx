import { CircleCheck, CircleDashed, CircleSlash, CircleX, LoaderCircle } from "lucide-react";
import type { Checks } from "../lib/types";

export function runState(status: string, conclusion: string | null): "ok" | "fail" | "run" | "off" {
  const s = status.toLowerCase();
  if (s !== "completed") return "run";
  const c = (conclusion ?? "").toLowerCase();
  if (c === "success") return "ok";
  if (["failure", "timed_out", "startup_failure", "action_required"].includes(c)) return "fail";
  return "off";
}

export function CiIcon({ status, conclusion, size = 15 }: { status: string; conclusion: string | null; size?: number }) {
  switch (runState(status, conclusion)) {
    case "ok":
      return <CircleCheck size={size} className="ci-ok" />;
    case "fail":
      return <CircleX size={size} className="ci-fail" />;
    case "run":
      return <LoaderCircle size={size} className="ci-run spin" />;
    default:
      return <CircleSlash size={size} className="ci-off" />;
  }
}

export function ChecksIcon({ checks, size = 14 }: { checks: Checks; size?: number }) {
  if (checks === "pass") return <CircleCheck size={size} className="ci-ok" />;
  if (checks === "fail") return <CircleX size={size} className="ci-fail" />;
  if (checks === "pending") return <LoaderCircle size={size} className="ci-run spin" />;
  return <CircleDashed size={size} className="ci-off" />;
}
