// Build-time switch for the Android app: `npm run build:android` builds with Vite's "android" mode.
export const IS_ANDROID: boolean = import.meta.env.MODE === 'android';
