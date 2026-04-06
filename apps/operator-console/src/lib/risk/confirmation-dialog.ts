export type ConfirmationDialogControl = "confirm" | "cancel";

export function resolveConfirmationTabLoop({
  key,
  shiftKey,
  activeControl,
}: {
  key: string;
  shiftKey: boolean;
  activeControl: ConfirmationDialogControl | null;
}): ConfirmationDialogControl | null {
  if (key !== "Tab") {
    return null;
  }
  if (shiftKey && activeControl === "confirm") {
    return "cancel";
  }
  if (!shiftKey && activeControl === "cancel") {
    return "confirm";
  }
  return null;
}

export function shouldDismissDangerConfirmation({
  key,
  isBusy,
}: {
  key: string;
  isBusy: boolean;
}): boolean {
  return key === "Escape" && !isBusy;
}
