const electronSvg = `data:image/svg+xml;utf8,${encodeURIComponent(
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48" fill="none">
    <rect x="6" y="6" width="36" height="36" rx="12" fill="#EEF4FF" />
    <path d="M16 24c0-6 3.6-10 8-10s8 4 8 10-3.6 10-8 10-8-4-8-10Z" stroke="#4F46E5" stroke-width="2.4" />
    <path d="M12 18c5.2 2.8 18.8 2.8 24 0" stroke="#4F46E5" stroke-width="2.4" stroke-linecap="round" />
    <path d="M12 30c5.2-2.8 18.8-2.8 24 0" stroke="#4F46E5" stroke-width="2.4" stroke-linecap="round" />
    <circle cx="24" cy="24" r="3.4" fill="#4F46E5" />
  </svg>`
)}`;

const fallbackIconMap = new Map([
  ['work review', '/icons/256x256.png'],
  ['work-review', '/icons/256x256.png'],
  ['work_review', '/icons/256x256.png'],
  ['electron', electronSvg],
  ['electron helper', electronSvg],
]);

function normalizeName(appName) {
  return typeof appName === 'string' ? appName.trim().toLowerCase() : '';
}

export function getFallbackAppIcon(appName) {
  const normalized = normalizeName(appName);
  if (!normalized) return null;

  if (fallbackIconMap.has(normalized)) {
    return fallbackIconMap.get(normalized);
  }

  if (normalized.includes('work review') || normalized.includes('work-review') || normalized.includes('work_review')) {
    return '/icons/256x256.png';
  }

  if (normalized.includes('electron')) {
    return electronSvg;
  }

  return null;
}

export function resolveAppIconSrc(appName, base64) {
  if (typeof base64 === 'string' && base64.length > 100) {
    return `data:image/png;base64,${base64}`;
  }

  return getFallbackAppIcon(appName);
}
