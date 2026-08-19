// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class NumaNodeMemory {
  final int freeBytes;
  final int nodeId;
  final int totalBytes;

  const NumaNodeMemory({
    required this.freeBytes,
    required this.nodeId,
    required this.totalBytes,
  });

  static NumaNodeMemory fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return NumaNodeMemory(
      freeBytes: (map['free_bytes'] as num).toInt(),
      nodeId: (map['node_id'] as num).toInt(),
      totalBytes: (map['total_bytes'] as num).toInt(),
    );
  }

  Map<String, dynamic> toJson() => {
    'free_bytes': freeBytes,
    'node_id': nodeId,
    'total_bytes': totalBytes,
  };
}
