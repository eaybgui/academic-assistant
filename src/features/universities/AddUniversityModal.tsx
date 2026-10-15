import { useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import { useNavigate } from 'react-router';
import Modal from '../../components/Modal';
import ErrorMessage from '../../components/ErrorMessage';
import Field from '../../components/forms/Field';
import TextInput from '../../components/forms/TextInput';
import Select from '../../components/forms/Select';
import { api, toCommandError } from '../../lib/api/client';
import type { CommandError, LmsCourseDto, LmsPlatform, LmsUniversityDto } from '../../lib/api/types';
import { useSync } from '../sync/useSync';
import CourseSelectionList from './CourseSelectionList';
import { PLATFORM_LABELS, connectionStatusText } from './labels';
import { useInvalidateUniversities } from './useInvalidateUniversities';

export interface AddUniversityModalProps {
  open: boolean;
  onClose: () => void;
}

const PLATFORM_OPTIONS = (Object.keys(PLATFORM_LABELS) as LmsPlatform[]).map((value) => ({
  value,
  label: PLATFORM_LABELS[value],
}));

const PRIMARY_BUTTON =
  'rounded bg-neutral-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-neutral-800 disabled:opacity-50';

/**
 * Three-step modal. Step 1 saves the university (the backend checks the URL belongs to the chosen
 * platform and rejects it otherwise). Step 2 opens the LMS sign-in window; closing the modal at
 * that point keeps the saved university, which the page then lists as "Not signed in". Step 3 loads
 * the courses (all pre-selected), saves the tracked ones, starts a sync and opens the tracker.
 */
export default function AddUniversityModal({ open, onClose }: AddUniversityModalProps) {
  const [name, setName] = useState('');
  const [platform, setPlatform] = useState<LmsPlatform>('moodle');
  const [url, setUrl] = useState('');
  const [nameError, setNameError] = useState<string | null>(null);
  const [urlError, setUrlError] = useState<string | null>(null);
  const [saved, setSaved] = useState<LmsUniversityDto | null>(null);
  const [connected, setConnected] = useState<LmsUniversityDto | null>(null);
  const [courses, setCourses] = useState<LmsCourseDto[] | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [error, setError] = useState<CommandError | null>(null);
  const invalidate = useInvalidateUniversities();
  const navigate = useNavigate();
  const { startSync } = useSync();

  const saveMutation = useMutation({
    mutationFn: () => api.addUniversity({ name: name.trim(), platform, baseUrl: url.trim() }),
    onSuccess: (university) => {
      setError(null);
      setSaved(university);
      invalidate();
    },
    onError: (e) => setError(toCommandError(e)),
  });

  const signInMutation = useMutation({
    mutationFn: (id: number) => api.lmsSignIn(id),
    onSuccess: (university) => {
      setError(null);
      setConnected(university);
    },
    onError: (e) => setError(toCommandError(e)),
    onSettled: invalidate,
  });

  const loadCoursesMutation = useMutation({
    mutationFn: (id: number) => api.lmsFetchCourses(id),
    onMutate: () => setError(null),
    onSuccess: (fetched) => {
      setCourses(fetched);
      setSelected(new Set(fetched.map((c) => c.id)));
    },
    onError: (e) => setError(toCommandError(e)),
  });

  const saveCoursesMutation = useMutation({
    mutationFn: async (id: number) => {
      await api.setTrackedCourses(id, [...selected]);
      // `startSync` never throws: a sync conflict is shown by the dashboard's sync state.
      await startSync(id);
    },
    onMutate: () => setError(null),
    onSuccess: () => {
      invalidate();
      onClose();
      void navigate('/');
    },
    onError: (e) => setError(toCommandError(e)),
  });

  function handleContinue() {
    const missingName = !name.trim();
    const missingUrl = !url.trim();
    setNameError(missingName ? 'University name is required.' : null);
    setUrlError(missingUrl ? 'Platform URL is required.' : null);
    if (missingName || missingUrl) return;
    setError(null);
    saveMutation.mutate();
  }

  const cancelButton = (
    <button
      type="button"
      onClick={onClose}
      className="rounded border border-neutral-300 px-3 py-1.5 text-sm text-neutral-700 hover:bg-neutral-50"
    >
      Cancel
    </button>
  );

  let footer = null;
  if (saved === null) {
    footer = (
      <>
        {cancelButton}
        <button type="button" disabled={saveMutation.isPending} onClick={handleContinue} className={PRIMARY_BUTTON}>
          {saveMutation.isPending ? 'Checking…' : 'Continue'}
        </button>
      </>
    );
  } else if (connected !== null && courses === null) {
    footer = (
      <button
        type="button"
        disabled={loadCoursesMutation.isPending}
        onClick={() => loadCoursesMutation.mutate(saved.id)}
        className={PRIMARY_BUTTON}
      >
        {loadCoursesMutation.isPending ? 'Loading courses…' : 'Choose courses'}
      </button>
    );
  } else if (courses !== null) {
    footer = (
      <button
        type="button"
        disabled={saveCoursesMutation.isPending}
        onClick={() => saveCoursesMutation.mutate(saved.id)}
        className={PRIMARY_BUTTON}
      >
        {saveCoursesMutation.isPending ? 'Saving…' : 'Save and sync'}
      </button>
    );
  }

  return (
    <Modal open={open} title="Add university" onClose={onClose} footer={footer}>
      {saved === null ? (
        <div className="flex flex-col gap-3">
          <Field label="Name" htmlFor="university-name" error={nameError ?? undefined}>
            <TextInput id="university-name" value={name} onChange={(e) => setName(e.target.value)} />
          </Field>
          <Field label="Platform" htmlFor="university-platform">
            <Select
              id="university-platform"
              options={PLATFORM_OPTIONS}
              value={platform}
              onChange={(e) => setPlatform(e.target.value as LmsPlatform)}
            />
          </Field>
          <Field
            label="Platform URL"
            htmlFor="university-url"
            error={urlError ?? undefined}
            hint="The address you use to open the platform in a browser."
          >
            <TextInput
              id="university-url"
              placeholder="https://aulavirtual.uv.es"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
            />
          </Field>
          <ErrorMessage error={error} />
        </div>
      ) : (
        <div className="flex flex-col gap-3">
          <p className="text-sm text-neutral-900">{saved.baseUrl}</p>
          {connected !== null ? (
            <p className="text-sm text-neutral-700">
              {connectionStatusText(connected.loginState, connected.accountLabel)}
            </p>
          ) : (
            <>
              <p className="text-sm text-neutral-700">
                Sign in with your university account in the window that opens.
              </p>
              <div>
                <button
                  type="button"
                  disabled={signInMutation.isPending}
                  onClick={() => signInMutation.mutate(saved.id)}
                  className={PRIMARY_BUTTON}
                >
                  {signInMutation.isPending ? 'Waiting for sign-in…' : 'Sign in'}
                </button>
              </div>
            </>
          )}
          {courses !== null ? (
            <CourseSelectionList courses={courses} selected={selected} onChange={setSelected} />
          ) : null}
          <ErrorMessage error={error} />
        </div>
      )}
    </Modal>
  );
}
