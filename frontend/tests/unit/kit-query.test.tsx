/** kit/query shares loads per key, reloads on invalidation, never lets an old load overwrite newer state and resets per scope. */
import {
  act,
  render,
  renderHook,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import {
  QueryCache,
  QueryProvider,
  keyText,
  useMutation,
  useQuery,
  useQueryCache,
  resetFallbackQueryCache,
} from "../../src/kit/query";

const deferred = <T,>() => {
  let resolve!: (value: T) => void, reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
};

describe("QueryCache", () => {
  it("shares one in-flight load and serves fresh data", async () => {
    const cache = new QueryCache();
    const load = vi.fn(async () => 1);
    const [a, b] = await Promise.all([
      cache.fetch(["k"], load),
      cache.fetch(["k"], load),
    ]);
    expect([a, b, load.mock.calls.length]).toEqual([1, 1, 1]);
    expect(await cache.fetch(["k"], load, 60_000)).toBe(1);
    expect(load).toHaveBeenCalledTimes(1);
    await cache.fetch(["k"], load, 0);
    expect(load).toHaveBeenCalledTimes(2);
    expect(cache.state(["k"])).toMatchObject({
      data: 1,
      status: "success",
      fetching: false,
    });
  });

  it("ignores a load that started before an invalidation, setData or disposal", async () => {
    const cache = new QueryCache();
    const slow = deferred<string>();
    const old = cache.fetch(["k"], () => slow.promise);
    cache.setData(["k"], "new");
    slow.resolve("old");
    await old;
    expect(cache.state(["k"]).data).toBe("new");
    const failing = deferred<string>();
    const failed = cache.fetch(["k"], () => failing.promise, 0);
    cache.invalidate(["k"]);
    failing.reject(new Error("late"));
    await expect(failed).rejects.toThrow("late");
    expect(cache.state(["k"])).toMatchObject({
      data: "new",
      status: "success",
    });
    const late = deferred<string>();
    const gone = cache.fetch(["x"], () => late.promise);
    cache.dispose();
    late.resolve("late");
    await gone;
    expect(cache.state(["x"]).status).toBe("idle");
  });

  it("records errors and invalidates by key prefix", async () => {
    const cache = new QueryCache();
    await expect(
      cache.fetch(["orders", 1], async () => {
        throw new Error("no");
      }),
    ).rejects.toThrow("no");
    expect(cache.state(["orders", 1]).status).toBe("error");
    await cache.fetch(["orders", 2], async () => 2);
    await cache.fetch(["ordersArchive"], async () => 3);
    await cache.fetch(["customers"], async () => 4);
    const seen = vi.fn();
    const stop = cache.subscribe(["orders", 2], seen);
    cache.invalidate(["orders"]);
    stop();
    expect(seen).toHaveBeenCalledTimes(1);
    expect(cache.state(["orders", 1]).status).toBe("idle");
    expect(cache.state(["orders", 2]).updatedAt).toBeUndefined();
    expect(cache.state(["ordersArchive"]).updatedAt).toBeDefined();
    cache.invalidate();
    expect(cache.state(["customers"]).updatedAt).toBeUndefined();
    expect(keyText(["a", 1, true, null])).toBe('["a",1,true,null]');
  });
});

describe("hooks", () => {
  const wrapper =
    (scope: string) =>
    ({ children }: { children: ReactNode }) => (
      <QueryProvider scope={scope}>{children}</QueryProvider>
    );

  it("loads once for equal keys and reloads after a mutation invalidates", async () => {
    let count = 0;
    const load = vi.fn(async () => ++count);
    function Reader({ name }: { name: string }) {
      const { data, fetching } = useQuery(["count"], load);
      return <p>{`${name}:${data ?? "-"}:${fetching}`}</p>;
    }
    function Writer() {
      const save = useMutation(async (value: number) => value * 2, {
        invalidates: [["count"]],
      });
      return (
        <button
          onClick={() => void save.run(4)}
        >{`${save.status}:${save.data ?? ""}`}</button>
      );
    }
    render(
      <QueryProvider scope="shop">
        <Reader name="a" />
        <Reader name="b" />
        <Writer />
      </QueryProvider>,
    );
    expect(await screen.findByText("a:1:false")).toBeInTheDocument();
    expect(screen.getByText("b:1:false")).toBeInTheDocument();
    expect(load).toHaveBeenCalledTimes(1);
    await act(async () => screen.getByRole("button").click());
    expect(await screen.findByText("a:2:false")).toBeInTheDocument();
    expect(screen.getByRole("button")).toHaveTextContent("success:8");
    expect(load).toHaveBeenCalledTimes(2);
  });

  it("keeps a failed query until refetch and reports mutation errors", async () => {
    let fail = true;
    const { result } = renderHook(
      () => ({
        query: useQuery(["k"], async () => {
          if (fail) throw new Error("down");
          return "up";
        }),
        save: useMutation(async () => {
          throw new Error("refused");
        }),
      }),
      { wrapper: wrapper("s") },
    );
    await waitFor(() => expect(result.current.query.status).toBe("error"));
    fail = false;
    let value: string | undefined;
    await act(async () => {
      value = await result.current.query.refetch();
    });
    expect(value).toBe("up");
    expect(result.current.query.data).toBe("up");
    fail = true;
    await act(async () => {
      value = await result.current.query.refetch();
    });
    expect(value).toBeUndefined();
    await act(async () => {
      await result.current.save.run(undefined).catch(() => {});
    });
    expect(result.current.save.status).toBe("error");
  });

  it("does not load while disabled and drops data when the scope changes", async () => {
    const load = vi.fn(async () => "data");
    let scope = "shop-a";
    const { result, rerender } = renderHook(
      ({ enabled }) => ({
        query: useQuery(["k"], load, { enabled }),
        cache: useQueryCache(),
      }),
      {
        initialProps: { enabled: false },
        wrapper: ({ children }) => (
          <QueryProvider scope={scope}>{children}</QueryProvider>
        ),
      },
    );
    expect(load).not.toHaveBeenCalled();
    rerender({ enabled: true });
    await waitFor(() => expect(result.current.query.data).toBe("data"));
    const first = result.current.cache;
    scope = "shop-b";
    rerender({ enabled: true });
    expect(result.current.cache).not.toBe(first);
    await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  });

  it("shares one fallback cache outside a provider until it is reset", async () => {
    const load = vi.fn(async () => 1);
    const first = renderHook(() => useQuery(["k"], load));
    await waitFor(() => expect(first.result.current.data).toBe(1));
    const second = renderHook(() => useQuery(["k"], load, { staleMs: 60_000 }));
    expect(second.result.current.data).toBe(1);
    expect(load).toHaveBeenCalledTimes(1);
    resetFallbackQueryCache();
    const third = renderHook(() => useQuery(["k"], load));
    await waitFor(() => expect(third.result.current.data).toBe(1));
    expect(load).toHaveBeenCalledTimes(2);
  });
  it("polls while mounted when refetchMs is set", async () => {
    vi.useFakeTimers();
    let count = 0;
    const { result, unmount } = renderHook(() =>
      useQuery(["poll"], async () => ++count, { refetchMs: 1000 }),
    );
    await vi.waitFor(() => expect(result.current.data).toBe(1));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    await vi.waitFor(() => expect(result.current.data).toBe(2));
    unmount();
    await vi.advanceTimersByTimeAsync(3000);
    expect(count).toBe(2);
    vi.useRealTimers();
  });
});
