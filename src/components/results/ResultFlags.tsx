export interface ResultFlagsProps {
  actionRequired: boolean;
  possibleChange: boolean;
}

/**
 * The "Action required" / "Possible change" chips shown for a result. Renders nothing when
 * neither flag is set. Shared by `ResultCard` (dashboard) and `ResultRow` (results table).
 */
export default function ResultFlags({ actionRequired, possibleChange }: ResultFlagsProps) {
  if (!actionRequired && !possibleChange) return null;

  return (
    <>
      {actionRequired ? (
        <span className="inline-flex items-center rounded-full bg-red-50 px-2 py-0.5 text-xs font-medium text-red-700">
          Action required
        </span>
      ) : null}
      {possibleChange ? (
        <span className="inline-flex items-center rounded-full bg-amber-50 px-2 py-0.5 text-xs font-medium text-amber-700">
          Possible change
        </span>
      ) : null}
    </>
  );
}
