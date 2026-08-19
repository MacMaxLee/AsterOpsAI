/// Mirrors `service::api::v1::history::RangeParam`
/// (rust_core/service/src/api/v1/history.rs) — wire values are
/// `last_hour`/`last_24h`/`last_7d`/`last_30d`/`custom`. Getting this wrong
/// is a real, previously-hit bug on the server side (U2's ADR 0007 records
/// serde's `rename_all = "snake_case"` silently producing `last24h` instead
/// of `last_24h` for a digit-containing variant name) — these values are
/// spelled out explicitly here rather than derived, for the same reason.
enum HistoryRangeParam {
  lastHour('last_hour'),
  last24h('last_24h'),
  last7d('last_7d'),
  last30d('last_30d'),
  custom('custom');

  final String wireValue;
  const HistoryRangeParam(this.wireValue);
}

final class HistoryQuery {
  final HistoryRangeParam range;
  final DateTime? from;
  final DateTime? to;

  const HistoryQuery(this.range, {this.from, this.to});

  String toQueryString() {
    final params = {'range': range.wireValue};
    if (from != null) params['from'] = from!.toUtc().toIso8601String();
    if (to != null) params['to'] = to!.toUtc().toIso8601String();
    return params.entries
        .map(
          (e) =>
              '${Uri.encodeQueryComponent(e.key)}=${Uri.encodeQueryComponent(e.value)}',
        )
        .join('&');
  }
}
