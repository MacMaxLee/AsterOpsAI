import 'api_failure.dart';

sealed class ApiResult<T> {
  const ApiResult();
}

final class ApiOk<T> extends ApiResult<T> {
  final T value;
  const ApiOk(this.value);
}

final class ApiErr<T> extends ApiResult<T> {
  final ApiFailure failure;
  const ApiErr(this.failure);
}
