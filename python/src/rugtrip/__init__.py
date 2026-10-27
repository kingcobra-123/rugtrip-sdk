"""RugTrip SDK: rug-risk verdicts for Robinhood Chain pools, a pre-trade policy gate and a live stream.

Ratings are risk opinions, not guarantees. Never treat LOWER_RISK as safe.
"""

from ._version import __version__
from .address import to_target
from .client import AsyncClient, Client
from .errors import ConfigError, RugTripError
from .models import (
    RATINGS,
    Alert,
    AlertEvent,
    Connected,
    Health,
    LpDetails,
    Pull,
    PullEvent,
    Reason,
    Reconnecting,
    RugsToday,
    Scorecard,
    ScorecardDay,
    ScorecardTotal,
    SimDetails,
    Stats,
    StreamEvent,
    TokenCheck,
    UnknownEvent,
    Verdict,
    VerdictDetails,
    VerdictEvent,
    parse_frame,
)
from .policy import DEFAULT_POLICY, GateResult, Policy, PolicyHit, gate, risk_rank, should_skip
from .stream import Stream, stream_url

__all__ = [
    "__version__",
    "Client",
    "AsyncClient",
    "Stream",
    "stream_url",
    "RugTripError",
    "ConfigError",
    "Policy",
    "PolicyHit",
    "GateResult",
    "DEFAULT_POLICY",
    "gate",
    "should_skip",
    "risk_rank",
    "to_target",
    "parse_frame",
    "RATINGS",
    "Alert",
    "AlertEvent",
    "Connected",
    "Health",
    "LpDetails",
    "Pull",
    "PullEvent",
    "Reason",
    "Reconnecting",
    "RugsToday",
    "Scorecard",
    "ScorecardDay",
    "ScorecardTotal",
    "SimDetails",
    "Stats",
    "StreamEvent",
    "TokenCheck",
    "UnknownEvent",
    "Verdict",
    "VerdictDetails",
    "VerdictEvent",
]
