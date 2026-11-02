import { useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '../../lib/api/client';
import type { PersonalTaskInput } from '../../lib/api/types';

/** Personal task mutations (create, update, done, delete); each refreshes every tracker query on success. */
export function usePersonalTaskMutations() {
  const queryClient = useQueryClient();
  const onSuccess = () => {
    void queryClient.invalidateQueries({ queryKey: ['tracker'] });
  };

  const create = useMutation({
    mutationFn: (input: PersonalTaskInput) => api.createPersonalTask(input),
    onSuccess,
  });
  const update = useMutation({
    mutationFn: ({ id, input }: { id: number; input: PersonalTaskInput }) =>
      api.updatePersonalTask(id, input),
    onSuccess,
  });
  const setDone = useMutation({
    mutationFn: ({ id, value }: { id: number; value: boolean }) =>
      api.setPersonalTaskDone(id, value),
    onSuccess,
  });
  const remove = useMutation({
    mutationFn: (id: number) => api.deletePersonalTask(id),
    onSuccess,
  });

  return { create, update, setDone, remove };
}
