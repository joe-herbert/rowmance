let version = $state(0);

export const savedQueriesInvalidator = {
  get version() {
    return version;
  },
  invalidate() {
    version++;
  },
};
