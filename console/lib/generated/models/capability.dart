// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

sealed class Capability {
  const Capability();

  static Capability fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    final tag = map['state'] as String;
    switch (tag) {
      case 'SUPPORTED':
        return CapabilitySupported.fromJson(map);
      case 'LIMITED':
        return CapabilityLimited.fromJson(map);
      case 'UNAVAILABLE':
        return CapabilityUnavailable.fromJson(map);
      case 'PERMISSION_REQUIRED':
        return CapabilityPermissionRequired.fromJson(map);
      default:
        throw FormatException('Unknown Capability tag: $tag');
    }
  }

  Map<String, dynamic> toJson();
}

final class CapabilitySupported extends Capability {
  const CapabilitySupported();

  static CapabilitySupported fromJson(Map<String, dynamic> map) {
    return CapabilitySupported();
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'SUPPORTED'};
}

final class CapabilityLimited extends Capability {
  final String reason;
  const CapabilityLimited({required this.reason});

  static CapabilityLimited fromJson(Map<String, dynamic> map) {
    return CapabilityLimited(reason: map['reason'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'LIMITED', 'reason': reason};
}

final class CapabilityUnavailable extends Capability {
  final String reason;
  const CapabilityUnavailable({required this.reason});

  static CapabilityUnavailable fromJson(Map<String, dynamic> map) {
    return CapabilityUnavailable(reason: map['reason'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'UNAVAILABLE', 'reason': reason};
}

final class CapabilityPermissionRequired extends Capability {
  final String reason;
  const CapabilityPermissionRequired({required this.reason});

  static CapabilityPermissionRequired fromJson(Map<String, dynamic> map) {
    return CapabilityPermissionRequired(reason: map['reason'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {
    'state': 'PERMISSION_REQUIRED',
    'reason': reason,
  };
}
