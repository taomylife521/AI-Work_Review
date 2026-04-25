import requests


class LocalApiClient:
    def __init__(self, base_url: str, token: str):
        self.base_url = base_url.rstrip("/")
        self.token = token

    def _params(self, extra=None):
        params = {"token": self.token}
        if extra:
            params.update(extra)
        return params

    def health(self):
        r = requests.get(f"{self.base_url}/health", timeout=10)
        return r.json()

    def device(self):
        r = requests.get(f"{self.base_url}/v1/device", params=self._params(), timeout=10)
        return r.json()

    def list_reports(self, limit=10):
        r = requests.get(
            f"{self.base_url}/v1/reports",
            params=self._params({"limit": limit}),
            timeout=10,
        )
        return r.json()

    def get_report(self, date_str: str, locale=None):
        params = self._params()
        if locale:
            params["locale"] = locale
        r = requests.get(f"{self.base_url}/v1/reports/{date_str}", params=params, timeout=30)
        return r.json()

    def generate_report(self, date_str: str, locale=None):
        body = {"date": date_str}
        if locale:
            body["locale"] = locale
        r = requests.post(
            f"{self.base_url}/v1/reports/generate",
            params=self._params(),
            json=body,
            timeout=120,
        )
        return r.json()
