// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'query_stat.dart';

sealed class GatedValueForArrayOfQueryStat {
  const GatedValueForArrayOfQueryStat();

  static GatedValueForArrayOfQueryStat fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    final tag = map['state'] as String;
    switch (tag) {
      case 'SUPPORTED':
        return GatedValueForArrayOfQueryStatSupported.fromJson(map);
      case 'LIMITED':
        return GatedValueForArrayOfQueryStatLimited.fromJson(map);
      case 'UNAVAILABLE':
        return GatedValueForArrayOfQueryStatUnavailable.fromJson(map);
      case 'PERMISSION_REQUIRED':
        return GatedValueForArrayOfQueryStatPermissionRequired.fromJson(map);
      default:
        throw FormatException(
          'Unknown GatedValueForArrayOfQueryStat tag: $tag',
        );
    }
  }

  Map<String, dynamic> toJson();
}

final class GatedValueForArrayOfQueryStatSupported
    extends GatedValueForArrayOfQueryStat {
  final List<QueryStat> value;
  const GatedValueForArrayOfQueryStatSupported({required this.value});

  static GatedValueForArrayOfQueryStatSupported fromJson(
    Map<String, dynamic> map,
  ) {
    return GatedValueForArrayOfQueryStatSupported(
      value: (map['value'] as List<dynamic>)
          .map((e) => QueryStat.fromJson(e))
          .toList(),
    );
  }

  @override
  Map<String, dynamic> toJson() => {
    'state': 'SUPPORTED',
    'value': value.map((e) => e.toJson()).toList(),
  };
}

final class GatedValueForArrayOfQueryStatLimited
    extends GatedValueForArrayOfQueryStat {
  final String reason;
  const GatedValueForArrayOfQueryStatLimited({required this.reason});

  static GatedValueForArrayOfQueryStatLimited fromJson(
    Map<String, dynamic> map,
  ) {
    return GatedValueForArrayOfQueryStatLimited(
      reason: map['reason'] as String,
    );
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'LIMITED', 'reason': reason};
}

final class GatedValueForArrayOfQueryStatUnavailable
    extends GatedValueForArrayOfQueryStat {
  final String reason;
  const GatedValueForArrayOfQueryStatUnavailable({required this.reason});

  static GatedValueForArrayOfQueryStatUnavailable fromJson(
    Map<String, dynamic> map,
  ) {
    return GatedValueForArrayOfQueryStatUnavailable(
      reason: map['reason'] as String,
    );
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'UNAVAILABLE', 'reason': reason};
}

final class GatedValueForArrayOfQueryStatPermissionRequired
    extends GatedValueForArrayOfQueryStat {
  final String reason;
  const GatedValueForArrayOfQueryStatPermissionRequired({required this.reason});

  static GatedValueForArrayOfQueryStatPermissionRequired fromJson(
    Map<String, dynamic> map,
  ) {
    return GatedValueForArrayOfQueryStatPermissionRequired(
      reason: map['reason'] as String,
    );
  }

  @override
  Map<String, dynamic> toJson() => {
    'state': 'PERMISSION_REQUIRED',
    'reason': reason,
  };
}
