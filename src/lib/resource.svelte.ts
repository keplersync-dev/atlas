/**
 * Loads async data inside a component and re-loads it when its inputs change.
 *
 *   const detail = resource(() => api.getInstance(id));
 *   {#if detail.loading} … {:else if detail.error} … {:else} {detail.data.name} {/if}
 *
 * How it works: the fetcher runs inside an $effect, so any reactive value it reads
 * *before its first await* (like the `id` prop) becomes a dependency. When that value
 * changes, the effect re-runs and fetches again. A version counter throws away
 * responses from older requests, so a slow old reply can never overwrite a newer one.
 *
 * Must be called while a component is being created (top level of <script>),
 * because $effect needs a component to belong to.
 */
export function resource<T>(fetcher: () => Promise<T>) {
  const state = $state<{
    data: T | undefined;
    loading: boolean;
    error: string | null;
  }>({
    data: undefined,
    loading: true,
    error: null,
  });
  let version = 0;

  function run(keepData: boolean) {
    const v = ++version;
    state.loading = true;
    state.error = null;
    if (!keepData) state.data = undefined;
    fetcher()
      .then((data) => {
        if (v === version) state.data = data;
      })
      .catch((e) => {
        if (v === version) state.error = (e as Error).message;
      })
      .finally(() => {
        if (v === version) state.loading = false;
      });
  }

  // Inputs changed → start fresh (don't show the previous item's data meanwhile).
  $effect(() => run(false));

  return {
    get data() {
      return state.data;
    },
    get loading() {
      return state.loading;
    },
    get error() {
      return state.error;
    },
    /** Fetch again after a change you made, keeping current data on screen meanwhile. */
    reload: () => run(true),
  };
}
