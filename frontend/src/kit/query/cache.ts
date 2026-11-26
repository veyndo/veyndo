/** In-memory query cache: one entry per key, shared in-flight loads, prefix invalidation and subscribers; nothing is persisted. */
export type QueryKey = readonly (string | number | boolean | null)[];
export type QueryState<T> = {
  data?: T;
  error?: unknown;
  status: "idle" | "loading" | "success" | "error";
  /** True while a load runs, also when older data is still shown. */
  fetching: boolean;
  updatedAt?: number;
};
type Entry = {
  state: QueryState<unknown>;
  load?: Promise<unknown>;
  generation: number;
  listeners: Set<() => void>;
};

/** The stable text form of a key; keys compare by value. */
export const keyText = (key: QueryKey) => JSON.stringify(key);

export class QueryCache {
  private entries = new Map<string, Entry>();
  private disposed = false;

  private entry(key: QueryKey): Entry {
    const text = keyText(key);
    let entry = this.entries.get(text);
    if (!entry) {
      entry = {
        state: { status: "idle", fetching: false },
        generation: 0,
        listeners: new Set(),
      };
      this.entries.set(text, entry);
    }
    return entry;
  }

  private set(entry: Entry, state: QueryState<unknown>) {
    entry.state = state;
    entry.listeners.forEach((listener) => listener());
  }

  state<T>(key: QueryKey): QueryState<T> {
    return this.entry(key).state as QueryState<T>;
  }

  subscribe(key: QueryKey, listener: () => void) {
    const entry = this.entry(key);
    entry.listeners.add(listener);
    return () => {
      entry.listeners.delete(listener);
    };
  }

  /**
   * Loads `key` unless a load is already running or the data is younger than
   * `staleMs`. Concurrent callers share one load; a load started before an
   * invalidation or reset never overwrites newer state.
   */
  fetch<T>(key: QueryKey, load: () => Promise<T>, staleMs = 0): Promise<T> {
    const entry = this.entry(key);
    if (entry.load) return entry.load as Promise<T>;
    const { state } = entry;
    if (
      state.status === "success" &&
      state.updatedAt !== undefined &&
      Date.now() - state.updatedAt < staleMs
    )
      return Promise.resolve(state.data as T);
    const generation = entry.generation;
    this.set(entry, {
      ...state,
      status: state.status === "success" ? "success" : "loading",
      fetching: true,
    });
    const current = () => !this.disposed && entry.generation === generation;
    const run = load().then(
      (data) => {
        if (current()) {
          entry.load = undefined;
          this.set(entry, {
            data,
            status: "success",
            fetching: false,
            updatedAt: Date.now(),
          });
        }
        return data;
      },
      (error: unknown) => {
        if (current()) {
          entry.load = undefined;
          this.set(entry, {
            ...entry.state,
            error,
            status: "error",
            fetching: false,
          });
        }
        throw error;
      },
    );
    entry.load = run;
    return run;
  }

  /** Replaces the data of `key`, for example with a mutation's answer. */
  setData<T>(key: QueryKey, data: T) {
    const entry = this.entry(key);
    entry.generation++;
    entry.load = undefined;
    this.set(entry, {
      data,
      status: "success",
      fetching: false,
      updatedAt: Date.now(),
    });
  }

  /**
   * Marks every entry whose key starts with `prefix` as stale and cancels the
   * effect of running loads; subscribers reload on notification.
   */
  invalidate(prefix: QueryKey = []) {
    const head = keyText(prefix).slice(0, -1);
    for (const [text, entry] of this.entries) {
      if (
        prefix.length &&
        !(text === keyText(prefix) || text.startsWith(head + ","))
      )
        continue;
      entry.generation++;
      entry.load = undefined;
      this.set(entry, {
        ...entry.state,
        status: entry.state.status === "error" ? "idle" : entry.state.status,
        updatedAt: undefined,
        fetching: false,
      });
    }
  }

  /** Forgets everything; later loads of this cache are ignored. */
  dispose() {
    this.disposed = true;
    for (const entry of this.entries.values()) entry.generation++;
    this.entries.clear();
  }
}
