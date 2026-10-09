import { open as tauriOpen, type OpenDialogOptions } from "@tauri-apps/plugin-dialog";

let openDialogs = 0;

/** Vrai pendant qu'un sélecteur natif est ouvert : l'island ne doit pas se replier. */
export const dialogOpen = () => openDialogs > 0;

export async function open(options: OpenDialogOptions) {
  openDialogs++;
  try {
    return await tauriOpen(options);
  } finally {
    // Laisse le temps à la fenêtre de reprendre le focus avant de réautoriser le repli.
    setTimeout(() => openDialogs--, 400);
  }
}
