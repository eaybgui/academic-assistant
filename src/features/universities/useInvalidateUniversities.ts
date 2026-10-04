import { useQueryClient } from '@tanstack/react-query';
import { queryKeys } from '../../lib/api/queryKeys';

/** Returns a function that refreshes everything derived from the university list. */
export function useInvalidateUniversities() {
  const queryClient = useQueryClient();
  return () => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.universities });
    void queryClient.invalidateQueries({ queryKey: queryKeys.catalog });
  };
}
