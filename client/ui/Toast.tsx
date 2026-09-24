import { createContext, useCallback, useContext, useState, type ReactNode } from "react";
import { Toast as T } from "radix-ui";
import { CircleAlert, CircleCheck, Info, X } from "lucide-react";

import { cx } from "./cx";
import s from "./overlay.module.css";

type ToastTone = "info" | "success" | "danger";

interface ToastMessage {
  id: number;
  title: ReactNode;
  description?: ReactNode | undefined;
  tone: ToastTone;
}

type Show = (t: { title: ReactNode; description?: ReactNode | undefined; tone?: ToastTone | undefined }) => void;

const Ctx = createContext<Show>(() => {});

let next = 1;

/** Transient confirmations in the bottom-right corner. Mounted by UiRoot. */
export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<ToastMessage[]>([]);

  const show = useCallback<Show>(({ title, description, tone = "info" }) => {
    setItems((cur) => [...cur, { id: next++, title, description, tone }]);
  }, []);

  const drop = (id: number) => setItems((cur) => cur.filter((t) => t.id !== id));

  return (
    <Ctx.Provider value={show}>
      <T.Provider swipeDirection="right" duration={5000}>
        {children}
        {items.map((t) => (
          <T.Root
            key={t.id}
            className={cx(s.toast, s[`toast_${t.tone}`])}
            onOpenChange={(open) => {
              if (!open) drop(t.id);
            }}
          >
            <span className={s.toastIcon} aria-hidden>
              {t.tone === "success" ? <CircleCheck /> : t.tone === "danger" ? <CircleAlert /> : <Info />}
            </span>
            <div className={s.toastText}>
              <T.Title className={s.toastTitle}>{t.title}</T.Title>
              {t.description && <T.Description className={s.toastDescription}>{t.description}</T.Description>}
            </div>
            <T.Close className={s.close} aria-label="Dismiss">
              <X />
            </T.Close>
          </T.Root>
        ))}
        <T.Viewport className={s.toastViewport} />
      </T.Provider>
    </Ctx.Provider>
  );
}

export function useToast(): Show {
  return useContext(Ctx);
}
