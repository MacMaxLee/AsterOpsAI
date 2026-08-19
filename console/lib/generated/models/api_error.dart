// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

sealed class ApiError {
  const ApiError();

  static ApiError fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    final tag = map['code'] as String;
    switch (tag) {
      case 'BAD_REQUEST':
        return ApiErrorBadRequest.fromJson(map);
      case 'NOT_FOUND':
        return ApiErrorNotFound.fromJson(map);
      case 'UNSUPPORTED':
        return ApiErrorUnsupported.fromJson(map);
      case 'PERMISSION_REQUIRED':
        return ApiErrorPermissionRequired.fromJson(map);
      case 'UNAVAILABLE':
        return ApiErrorUnavailable.fromJson(map);
      case 'INTERNAL':
        return ApiErrorInternal.fromJson(map);
      default:
        throw FormatException('Unknown ApiError tag: $tag');
    }
  }

  Map<String, dynamic> toJson();
}

final class ApiErrorBadRequest extends ApiError {
  final String message;
  const ApiErrorBadRequest({required this.message});

  static ApiErrorBadRequest fromJson(Map<String, dynamic> map) {
    return ApiErrorBadRequest(message: map['message'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'code': 'BAD_REQUEST', 'message': message};
}

final class ApiErrorNotFound extends ApiError {
  const ApiErrorNotFound();

  static ApiErrorNotFound fromJson(Map<String, dynamic> map) {
    return ApiErrorNotFound();
  }

  @override
  Map<String, dynamic> toJson() => {'code': 'NOT_FOUND'};
}

final class ApiErrorUnsupported extends ApiError {
  final String message;
  const ApiErrorUnsupported({required this.message});

  static ApiErrorUnsupported fromJson(Map<String, dynamic> map) {
    return ApiErrorUnsupported(message: map['message'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'code': 'UNSUPPORTED', 'message': message};
}

final class ApiErrorPermissionRequired extends ApiError {
  final String message;
  const ApiErrorPermissionRequired({required this.message});

  static ApiErrorPermissionRequired fromJson(Map<String, dynamic> map) {
    return ApiErrorPermissionRequired(message: map['message'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {
    'code': 'PERMISSION_REQUIRED',
    'message': message,
  };
}

final class ApiErrorUnavailable extends ApiError {
  final String message;
  const ApiErrorUnavailable({required this.message});

  static ApiErrorUnavailable fromJson(Map<String, dynamic> map) {
    return ApiErrorUnavailable(message: map['message'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'code': 'UNAVAILABLE', 'message': message};
}

final class ApiErrorInternal extends ApiError {
  const ApiErrorInternal();

  static ApiErrorInternal fromJson(Map<String, dynamic> map) {
    return ApiErrorInternal();
  }

  @override
  Map<String, dynamic> toJson() => {'code': 'INTERNAL'};
}
