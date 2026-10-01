import hmac
import secrets
import time
from functools import wraps

from flask import Blueprint, abort, current_app, g, jsonify, request

from ..cache import cache

bp = Blueprint("auth", __name__, url_prefix="/auth")

USERS = {"ada": "pbkdf2:correct-horse", "grace": "pbkdf2:battery-staple"}
MIN_PASSWORD_LENGTH = 8


def issue(user: str) -> str:
    token = secrets.token_urlsafe(32)
    cache.put(f"token:{token}", {"user": user, "issued": time.time()})
    return token


def require_token(view):
    @wraps(view)
    def guarded(*args, **kwargs):
        header = request.headers.get("Authorization", "")
        token = header.removeprefix("Bearer ").strip()
        session = cache.get(f"token:{token}") if token else None
        if session is None:
            abort(401, description="missing or expired bearer token")
        if time.time() - session["issued"] > current_app.config["TOKEN_TTL_S"]:
            cache.drop(f"token:{token}")
            abort(401, description="missing or expired bearer token")
        g.user = session["user"]
        return view(*args, **kwargs)

    return guarded


@bp.post("/login")
def login():
    body = request.get_json(silent=True) or {}
    user = str(body.get("user", ""))
    password = str(body.get("password", ""))
    if len(password) < MIN_PASSWORD_LENGTH:
        abort(422, description=f"password is shorter than {MIN_PASSWORD_LENGTH}")
    expected = USERS.get(user)
    if expected is None or not hmac.compare_digest(expected, f"pbkdf2:{password}"):
        abort(401, description="unknown user or wrong password")
    return jsonify({"token": issue(user), "expires_in": current_app.config["TOKEN_TTL_S"]})


@bp.post("/logout")
@require_token
def logout():
    token = request.headers.get("Authorization", "").removeprefix("Bearer ").strip()
    cache.drop(f"token:{token}")
    return "", 204
