import type { AppError } from "../bindings";

type CommandResult<T> = { status: "ok"; data: T } | { status: "error"; error: AppError };

/** Error thrown by {@link unwrap}, carrying the typed backend error. */
export class CommandError extends Error {
  constructor(readonly appError: AppError) {
    super(appError.message);
    this.name = "CommandError";
  }
}

/** Resolves to the command's data, or throws a {@link CommandError}. */
export async function unwrap<T>(result: Promise<CommandResult<T>>): Promise<T> {
  const settled = await result;
  if (settled.status === "error") {
    throw new CommandError(settled.error);
  }
  return settled.data;
}

/** User-facing (French) message for an error thrown while calling the backend. */
export function errorMessage(error: unknown): string {
  if (error instanceof CommandError) {
    switch (error.appError.kind) {
      case "not_found":
        return "Cet élément n'existe plus. Recharge l'écran.";
      case "validation":
        return `Saisie refusée : ${error.appError.message}.`;
      default:
        return `Erreur inattendue : ${error.appError.message}.`;
    }
  }
  return `Erreur inattendue : ${String(error)}.`;
}
