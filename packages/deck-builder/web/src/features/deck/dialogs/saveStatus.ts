// Lets a dialog with editable fields say whether its last edit was saved.
// useAttempt reports through it; dialogs that do not provide it ignore it.

import { createContext, useContext } from "react";

export type SaveStatus = "idle" | "saving" | "saved";
export const SaveStatusContext = /* @__PURE__ */ createContext<(status: SaveStatus) => void>(() => {});
export const useSaveStatusReport = () => useContext(SaveStatusContext);
