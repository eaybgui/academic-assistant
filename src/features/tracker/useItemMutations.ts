import { useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '../../lib/api/client';

/** Per-item tracker mutations (done, hidden, note, dismiss change); each refreshes every tracker query on success. */
export function useItemMutations() {
  const queryClient = useQueryClient();
  const onSuccess = () => {
    void queryClient.invalidateQueries({ queryKey: ['tracker'] });
  };

  const setDone = useMutation({
    mutationFn: ({ id, value }: { id: number; value: boolean }) => api.setItemDone(id, value),
    onSuccess,
  });
  const setHidden = useMutation({
    mutationFn: ({ id, value }: { id: number; value: boolean }) => api.setItemHidden(id, value),
    onSuccess,
  });
  const setNote = useMutation({
    mutationFn: ({ id, note }: { id: number; note: string | null }) => api.setItemNote(id, note),
    onSuccess,
  });
  const dismissChange = useMutation({
    mutationFn: (id: number) => api.dismissItemChange(id),
    onSuccess,
  });

  return { setDone, setHidden, setNote, dismissChange };
}
