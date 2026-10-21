import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { loadPetAnimation, type PetAnimation } from "./gif";

const cache = new Map<string, Promise<PetAnimation>>();

/** Lê e processa o GIF uma vez por nome; as próximas chamadas reaproveitam o resultado. */
export function loadPet(name: string): Promise<PetAnimation> {
  let promise = cache.get(name);
  if (!promise) {
    promise = invoke<ArrayBuffer>("read_pet", { name }).then(loadPetAnimation);
    promise.catch(() => cache.delete(name));
    cache.set(name, promise);
  }
  return promise;
}

export function forgetPet(name: string) {
  cache.delete(name);
}

/** Nomes dos GIFs importados. */
export function usePetLibrary() {
  const [names, setNames] = useState<string[] | null>(null);
  const refresh = useCallback(() => {
    invoke<string[]>("list_pets")
      .then(setNames)
      .catch(() => setNames([]));
  }, []);
  useEffect(refresh, [refresh]);
  return { names, refresh };
}

export function usePetAnimation(name: string | null) {
  const [anim, setAnim] = useState<PetAnimation | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    setAnim(null);
    setError(null);
    if (!name) return;
    let alive = true;
    loadPet(name)
      .then((a) => alive && setAnim(a))
      .catch((e) => alive && setError(String(e)));
    return () => {
      alive = false;
    };
  }, [name]);
  return { anim, error };
}
