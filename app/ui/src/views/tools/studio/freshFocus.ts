// A name field that takes the focus (all of it selected) when the thing it names was just added
// under My mode, so a new variable, Function or Timeline can be named at once (UX review M5).

import { useEffect, useRef } from "react";
import { useStudio } from "./store";

export function useFreshFocus<T extends HTMLInputElement>(id: string | undefined) {
  const ref = useRef<T>(null);
  const fresh = useStudio((s) => !!id && s.renameNew === id);
  useEffect(() => {
    if (!fresh || !ref.current) return;
    ref.current.focus();
    ref.current.select();
    useStudio.getState().setRenameNew(null);
  }, [fresh]);
  return ref;
}
