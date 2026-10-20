import { ref } from 'vue'

const searchValue = ref('')
const filters = ref({
  syncedLyricsTracks: true,
  plainLyricsTracks: true,
  instrumentalTracks: true,
  noLyricsTracks: true,
})

export function useSearchLibrary() {
  // The parameter must not be named `filters`: it would shadow the module-level
  // ref, so `filters.value = filters` would assign the argument to itself and
  // nothing outside this function would ever change.
  const setSearch = (text, nextFilters) => {
    searchValue.value = text
    filters.value = nextFilters
  }

  return {
    searchValue,
    filters,
    setSearch,
  }
}
