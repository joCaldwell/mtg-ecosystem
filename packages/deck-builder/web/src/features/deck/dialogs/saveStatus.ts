// Lets a dialog with editable fields say whether its last edit was saved.
// useAttempt reports through it; dialogs that do not provide it ignore it.

import { createContext, useContext } from "react";

export type SaveStatus = "idle" | "saving" | "saved" | "failed";
/** `message` is the failed request's own error, for "failed". */
export type SaveStatusReport = (status: SaveStatus, message?: string) => void;
export const SaveStatusContext = /* @__PURE__ */ createContext<SaveStatusReport>(() => {});
export const useSaveStatusReport = () => useContext(SaveStatusContext);
