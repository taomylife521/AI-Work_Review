function formatIsoDate(date) {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}

export function shiftIsoDate(dateValue, offsetDays) {
  const next = new Date(`${dateValue}T12:00:00`);
  next.setDate(next.getDate() + offsetDays);
  return formatIsoDate(next);
}

export function createReportRequestSnapshot(requestId, selectedDate, locale) {
  return {
    requestId,
    targetDate: selectedDate,
    targetLocale: locale,
    targetCacheKey: `${selectedDate}:${locale}`,
  };
}

export function createReportGenerationOwnership() {
  let ownerRequestId = null;

  return {
    claim(requestId, reportGenerating) {
      if (ownerRequestId !== null || reportGenerating) return false;
      ownerRequestId = requestId;
      return true;
    },
    release(requestId) {
      if (ownerRequestId !== requestId) return false;
      ownerRequestId = null;
      return true;
    },
  };
}
