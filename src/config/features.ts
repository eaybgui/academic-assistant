import { IS_ANDROID } from './platform';

// Build-time switch for the email section; off unless VITE_EMAIL_FEATURES is exactly "true", and never on Android.
export const EMAIL_FEATURES_ENABLED: boolean = import.meta.env.VITE_EMAIL_FEATURES === 'true' && !IS_ANDROID;
