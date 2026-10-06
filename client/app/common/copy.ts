/**
 * Put text on the clipboard.
 *
 * The Clipboard API is there only in a secure context, HTTPS or localhost.
 * Spork is often reached over the LAN on plain HTTP, where it is not, so the
 * text is put in a hidden field, selected and copied the older way. Rejects
 * when neither works, so the caller can say to copy it by hand.
 */
export async function copyText(text: string): Promise<void> {
  if (window.isSecureContext && navigator.clipboard) return navigator.clipboard.writeText(text);
  const field = document.createElement("textarea");
  field.value = text;
  field.readOnly = true;
  field.setAttribute("aria-hidden", "true");
  field.style.position = "fixed";
  field.style.opacity = "0";
  document.body.append(field);
  field.select();
  try {
    if (!document.execCommand("copy")) throw new Error("the browser refused to copy");
  } finally {
    field.remove();
  }
}
