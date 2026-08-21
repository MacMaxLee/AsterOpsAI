// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'ai_explanation.dart';

sealed class GatedValueForAiExplanation {
  const GatedValueForAiExplanation();

  static GatedValueForAiExplanation fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    final tag = map['state'] as String;
    switch (tag) {
      case 'SUPPORTED':
        return GatedValueForAiExplanationSupported.fromJson(map);
      case 'LIMITED':
        return GatedValueForAiExplanationLimited.fromJson(map);
      case 'UNAVAILABLE':
        return GatedValueForAiExplanationUnavailable.fromJson(map);
      case 'PERMISSION_REQUIRED':
        return GatedValueForAiExplanationPermissionRequired.fromJson(map);
      default:
        throw FormatException('Unknown GatedValueForAiExplanation tag: $tag');
    }
  }

  Map<String, dynamic> toJson();
}

final class GatedValueForAiExplanationSupported
    extends GatedValueForAiExplanation {
  final AiExplanation value;
  const GatedValueForAiExplanationSupported({required this.value});

  static GatedValueForAiExplanationSupported fromJson(
    Map<String, dynamic> map,
  ) {
    return GatedValueForAiExplanationSupported(
      value: AiExplanation.fromJson(map['value']),
    );
  }

  @override
  Map<String, dynamic> toJson() => {
    'state': 'SUPPORTED',
    'value': value.toJson(),
  };
}

final class GatedValueForAiExplanationLimited
    extends GatedValueForAiExplanation {
  final String reason;
  const GatedValueForAiExplanationLimited({required this.reason});

  static GatedValueForAiExplanationLimited fromJson(Map<String, dynamic> map) {
    return GatedValueForAiExplanationLimited(reason: map['reason'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'LIMITED', 'reason': reason};
}

final class GatedValueForAiExplanationUnavailable
    extends GatedValueForAiExplanation {
  final String reason;
  const GatedValueForAiExplanationUnavailable({required this.reason});

  static GatedValueForAiExplanationUnavailable fromJson(
    Map<String, dynamic> map,
  ) {
    return GatedValueForAiExplanationUnavailable(
      reason: map['reason'] as String,
    );
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'UNAVAILABLE', 'reason': reason};
}

final class GatedValueForAiExplanationPermissionRequired
    extends GatedValueForAiExplanation {
  final String reason;
  const GatedValueForAiExplanationPermissionRequired({required this.reason});

  static GatedValueForAiExplanationPermissionRequired fromJson(
    Map<String, dynamic> map,
  ) {
    return GatedValueForAiExplanationPermissionRequired(
      reason: map['reason'] as String,
    );
  }

  @override
  Map<String, dynamic> toJson() => {
    'state': 'PERMISSION_REQUIRED',
    'reason': reason,
  };
}
