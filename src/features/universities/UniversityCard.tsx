import { useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import ErrorMessage from '../../components/ErrorMessage';
import ConfirmDialog from '../../components/forms/ConfirmDialog';
import { PencilIcon, TrashIcon } from '../../components/forms/icons';
import { api, toCommandError } from '../../lib/api/client';
import type { CommandError, LmsUniversityDto } from '../../lib/api/types';
import { PLATFORM_LABELS, connectionStatusText } from './labels';
import RenameUniversityModal from './RenameUniversityModal';
import { useInvalidateUniversities } from './useInvalidateUniversities';

export interface UniversityCardProps {
  university: LmsUniversityDto;
}

const TEXT_BUTTON = 'text-sm text-neutral-700 hover:underline disabled:opacity-50';

/** One university: connection status, sign-in/check/sign-out actions, rename and delete. */
export default function UniversityCard({ university }: UniversityCardProps) {
  const { id, name, loginState } = university;
  const invalidate = useInvalidateUniversities();

  const [actionError, setActionError] = useState<CommandError | null>(null);
  const [deleteError, setDeleteError] = useState<CommandError | null>(null);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [renameOpen, setRenameOpen] = useState(false);
  // Part of the rename modal's `key`, so each open starts from the current name.
  const [renameOpenCount, setRenameOpenCount] = useState(0);

  function connectionMutation(action: (id: number) => Promise<LmsUniversityDto>) {
    return {
      mutationFn: () => action(id),
      onMutate: () => setActionError(null),
      onError: (e: unknown) => setActionError(toCommandError(e)),
      onSettled: invalidate,
    };
  }

  const signInMutation = useMutation(connectionMutation(api.lmsSignIn));
  const signOutMutation = useMutation(connectionMutation(api.lmsSignOut));
  const checkMutation = useMutation(connectionMutation(api.lmsCheckSession));

  const deleteMutation = useMutation({
    mutationFn: () => api.removeUniversity(id),
    onSuccess: () => {
      setDeleteOpen(false);
      invalidate();
    },
    onError: (e) => setDeleteError(toCommandError(e)),
  });

  const busy = signInMutation.isPending || signOutMutation.isPending || checkMutation.isPending;
  const { courses } = university;

  function openRename() {
    setRenameOpenCount((count) => count + 1);
    setRenameOpen(true);
  }

  function openDelete() {
    setDeleteError(null);
    setDeleteOpen(true);
  }

  return (
    <li className="rounded border border-neutral-200 bg-white px-4 py-3">
      <div className="flex items-center justify-between gap-4">
        <div>
          <p className="font-bold text-neutral-900">{name}</p>
          <p className="text-sm text-neutral-500">
            {PLATFORM_LABELS[university.platform]} · {university.baseUrl}
          </p>
          <p className={`text-sm ${loginState === 'ok' ? 'text-neutral-600' : 'text-red-600'}`}>
            {connectionStatusText(loginState, university.accountLabel)}
          </p>
          {courses.total > 0 ? (
            <p className="text-sm text-neutral-500">
              {courses.tracked} of {courses.total} courses tracked
            </p>
          ) : null}
          <ErrorMessage error={actionError} />
        </div>

        <div className="flex items-center gap-3">
          {loginState !== 'ok' ? (
            <button
              type="button"
              aria-label={`Sign in to ${name}`}
              disabled={busy}
              onClick={() => signInMutation.mutate()}
              className={TEXT_BUTTON}
            >
              {signInMutation.isPending ? 'Waiting for sign-in…' : 'Sign in'}
            </button>
          ) : null}
          {loginState !== 'never' ? (
            <>
              <button
                type="button"
                aria-label={`Check connection to ${name}`}
                disabled={busy}
                onClick={() => checkMutation.mutate()}
                className={TEXT_BUTTON}
              >
                {checkMutation.isPending ? 'Checking…' : 'Check connection'}
              </button>
              <button
                type="button"
                aria-label={`Sign out of ${name}`}
                disabled={busy}
                onClick={() => signOutMutation.mutate()}
                className={TEXT_BUTTON}
              >
                Sign out
              </button>
            </>
          ) : null}
          <button
            type="button"
            aria-label={`Rename ${name}`}
            onClick={openRename}
            className="text-neutral-400 hover:text-neutral-700"
          >
            <PencilIcon />
          </button>
          <button
            type="button"
            aria-label={`Delete ${name}`}
            onClick={openDelete}
            className="text-neutral-400 hover:text-red-600"
          >
            <TrashIcon />
          </button>
        </div>
      </div>

      <RenameUniversityModal
        key={renameOpenCount}
        open={renameOpen}
        university={university}
        onClose={() => setRenameOpen(false)}
      />

      <ConfirmDialog
        open={deleteOpen}
        title="Delete university"
        message="Deletes this university, its courses and your saved sign-in."
        error={deleteError}
        pending={deleteMutation.isPending}
        onCancel={() => setDeleteOpen(false)}
        onConfirm={() => deleteMutation.mutate()}
      />
    </li>
  );
}
