class NotFoundError(Exception):
    def __init__(self, kind: str, id: str | int) -> None:
        super().__init__(f"{kind} {id} not found")
        self.kind = kind
        self.id = id


class ConflictError(Exception):
    def __init__(self, state: str, message: str) -> None:
        super().__init__(message)
        self.state = state


class ValidationError(Exception):
    def __init__(self, field: str, message: str) -> None:
        super().__init__(f"{field}: {message}")
        self.field = field


class UpstreamError(Exception):
    def __init__(self, system: str, message: str) -> None:
        super().__init__(f"{system}: {message}")
        self.system = system
