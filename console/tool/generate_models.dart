// Codegen for /schemas/*.schema.json (draft-07, emitted by
// rust_core/contracts/src/bin/emit_schemas.rs). Run: `dart run
// tool/generate_models.dart`. Output is committed (same convention as
// /schemas/*.json itself being committed) so CI's drift job can diff a
// fresh run against what's checked in.
//
// Deliberately skips envelope_*.schema.json: every envelope shares one
// structural shape ({success, timestamp, request_id, data, error}) that
// only varies by the type of `data` — that's handled by one hand-written
// generic `Envelope<T>` in lib/api/envelope.dart (mirroring how
// contracts::Envelope<T> itself is hand-written Rust, not schema-generated;
// schemars only monomorphizes it away per-T when *emitting* schemas). Every
// other named type is generated — no hand-written DTOs.
//
// Schema shapes this generator understands, confirmed by reading every
// committed schema file before writing this generator (not guessed):
//   - plain object: `type: object` + `properties` + `required`
//   - internally-tagged union: `oneOf`, every branch has a literal
//     single-value string enum discriminator property (`state` or `code`)
//   - plain string enum: `type: string` + `enum`
//   - `$ref: "#/definitions/X"`
//   - array: `type: array` + `items`
//   - map: `type: object` + `additionalProperties` (no `properties`)
//   - nullable: `type: [X, "null"]` (this codebase never uses `anyOf` for
//     nullability outside envelope_*.schema.json, which is skipped)
//   - primitives: string (plus `format: date-time` -> DateTime), integer,
//     number, boolean

import 'dart:convert';
import 'dart:io';

void main() {
  final schemasDir = Directory('../schemas');
  if (!schemasDir.existsSync()) {
    stderr.writeln('error: ${schemasDir.path} not found (run from console/)');
    exit(2);
  }

  final registry = <String, Map<String, dynamic>>{};
  final files =
      schemasDir
          .listSync()
          .whereType<File>()
          .where((f) => f.path.endsWith('.schema.json'))
          .where((f) => !f.uri.pathSegments.last.startsWith('envelope_'))
          .toList()
        ..sort((a, b) => a.path.compareTo(b.path));

  for (final file in files) {
    final root = jsonDecode(file.readAsStringSync()) as Map<String, dynamic>;
    _register(registry, root);
    final defs = root['definitions'] as Map<String, dynamic>?;
    if (defs != null) {
      // Nested definitions don't repeat their own `title`; the definitions
      // map's own key is the name.
      defs.forEach((name, node) {
        _registerNamed(registry, name, node as Map<String, dynamic>);
      });
    }
  }

  final outDir = Directory('lib/generated/models');
  if (outDir.existsSync()) {
    outDir.deleteSync(recursive: true);
  }
  outDir.createSync(recursive: true);

  final names = registry.keys.toList()..sort();
  for (final name in names) {
    final node = registry[name]!;
    final dartName = _dartName(name);
    final source = _generateType(dartName, node);
    File('${outDir.path}/${_snakeFileName(dartName)}.dart')
        .writeAsStringSync(source);
  }

  final barrel = StringBuffer()
    ..writeln('// GENERATED CODE - DO NOT EDIT BY HAND.')
    ..writeln('// Regenerate: dart run tool/generate_models.dart')
    ..writeln();
  for (final name in names) {
    final dartName = _dartName(name);
    barrel.writeln("export '${_snakeFileName(dartName)}.dart';");
  }
  File('${outDir.path}/models.dart').writeAsStringSync(barrel.toString());

  stdout.writeln('generated ${names.length} model files -> ${outDir.path}');
}

void _register(
  Map<String, Map<String, dynamic>> registry,
  Map<String, dynamic> node,
) {
  final title = node['title'] as String?;
  if (title == null) {
    throw StateError('schema root has no title: $node');
  }
  _registerNamed(registry, title, node);
}

void _registerNamed(
  Map<String, Map<String, dynamic>> registry,
  String name,
  Map<String, dynamic> node,
) {
  final existing = registry[name];
  if (existing != null) {
    // Definitions repeat verbatim across files (e.g. MetricValue_for_double
    // shows up standalone and inlined into every snapshot that uses it) —
    // assert that structurally (map key order isn't guaranteed consistent
    // between a standalone file and a nested `definitions` copy, so a raw
    // string compare would false-positive), don't silently trust it.
    if (!_deepEquals(
      _withoutDescription(existing),
      _withoutDescription(node),
    )) {
      throw StateError('conflicting definitions for $name');
    }
    return;
  }
  registry[name] = node;
}

Map<String, dynamic> _withoutDescription(Map<String, dynamic> node) {
  final copy = Map<String, dynamic>.from(node);
  copy.remove('description');
  copy.remove('title');
  copy.remove(
    '\$schema',
  ); // only ever present on a document root, not a nested definition
  return copy;
}

bool _deepEquals(dynamic a, dynamic b) {
  if (a is Map && b is Map) {
    if (a.length != b.length) return false;
    for (final key in a.keys) {
      if (!b.containsKey(key)) return false;
      if (!_deepEquals(a[key], b[key])) return false;
    }
    return true;
  }
  if (a is List && b is List) {
    if (a.length != b.length) return false;
    for (var i = 0; i < a.length; i++) {
      if (!_deepEquals(a[i], b[i])) return false;
    }
    return true;
  }
  return a == b;
}

// "MetricValue_for_double" -> "MetricValueForDouble"
// "HistoryResponse_for_CpuHistoryPoint" -> "HistoryResponseForCpuHistoryPoint"
// Each underscore-separated part already carries the casing it should keep
// (schemars names are Rust-identifier-cased) — only the leading letter of
// each part needs capitalizing.
String _dartName(String schemaName) {
  final parts = schemaName.split('_').where((p) => p.isNotEmpty);
  return parts.map((p) => p[0].toUpperCase() + p.substring(1)).join();
}

// "SAMPLE_GAP" -> "SampleGap", "PERMISSION_REQUIRED" -> "PermissionRequired"
// Tag values are SCREAMING_SNAKE_CASE on the wire, so (unlike _dartName)
// the remainder of each part must be lowercased before capitalizing.
String _pascalFromScreamingSnake(String tagValue) {
  final parts = tagValue.split('_').where((p) => p.isNotEmpty);
  return parts
      .map((p) => p[0].toUpperCase() + p.substring(1).toLowerCase())
      .join();
}

String _snakeFileName(String dartName) {
  final buf = StringBuffer();
  for (var i = 0; i < dartName.length; i++) {
    final c = dartName[i];
    if (c.toUpperCase() == c && c.toLowerCase() != c && i > 0) {
      buf.write('_');
    }
    buf.write(c.toLowerCase());
  }
  return buf.toString();
}

String _refName(String ref) {
  const prefix = '#/definitions/';
  if (!ref.startsWith(prefix)) {
    throw StateError('unsupported \$ref: $ref');
  }
  return _dartName(ref.substring(prefix.length));
}

class _FieldType {
  final String dartType; // without trailing '?'
  final bool nullable;
  final String Function(String jsonExpr) fromJson;
  final String Function(String dartExpr) toJson;

  _FieldType({
    required this.dartType,
    required this.nullable,
    required this.fromJson,
    required this.toJson,
  });

  String get dartTypeWithNullability => nullable ? '$dartType?' : dartType;

  String fromJsonNullable(String jsonExpr) {
    if (!nullable) return fromJson(jsonExpr);
    return '$jsonExpr == null ? null : (${fromJson(jsonExpr)})';
  }

  // Every toJson closure above produces either the bare expression back
  // (identity, for already-JSON-safe primitives) or `<dartExpr>.something`
  // (a direct method/property access chain) — never anything more exotic.
  // Dart won't promote a nullable *field* the way it promotes a local
  // variable, so `dartExpr == null ? null : dartExpr.foo()` still fails
  // analysis even inside the non-null branch; swapping the leading `.` for
  // `?.` sidesteps that without needing promotion at all.
  String toJsonNullable(String dartExpr) {
    if (!nullable) return toJson(dartExpr);
    final nonNull = toJson(dartExpr);
    if (nonNull == dartExpr) return dartExpr;
    final prefix = '$dartExpr.';
    if (!nonNull.startsWith(prefix)) {
      throw StateError('toJson output not in the expected shape: $nonNull');
    }
    return '$dartExpr?.${nonNull.substring(prefix.length)}';
  }
}

_FieldType _resolveType(Map<String, dynamic> schema) {
  final rawType = schema['type'];
  var nullable = false;
  dynamic type = rawType;
  if (rawType is List) {
    final nonNull = rawType.where((t) => t != 'null').toList();
    if (nonNull.length != 1 || nonNull.length == rawType.length) {
      throw StateError('unsupported type union: $rawType');
    }
    nullable = true;
    type = nonNull.first;
  }

  if (schema.containsKey('\$ref')) {
    final name = _refName(schema['\$ref'] as String);
    return _FieldType(
      dartType: name,
      nullable: nullable,
      fromJson: (e) => '$name.fromJson($e)',
      toJson: (e) => '$e.toJson()',
    );
  }

  // schemars wraps a `$ref` in `allOf: [{$ref: ...}]` whenever the property
  // itself also carries a sibling `description` (draft-07 doesn't allow
  // description next to $ref) — structurally the same as a bare $ref.
  if (schema['allOf'] is List && (schema['allOf'] as List).length == 1) {
    final inner = (schema['allOf'] as List).first as Map<String, dynamic>;
    if (inner.containsKey('\$ref')) {
      final name = _refName(inner['\$ref'] as String);
      return _FieldType(
        dartType: name,
        nullable: nullable,
        fromJson: (e) => '$name.fromJson($e)',
        toJson: (e) => '$e.toJson()',
      );
    }
  }

  if (type == 'array') {
    final item = _resolveType(schema['items'] as Map<String, dynamic>);
    final dartType = 'List<${item.dartTypeWithNullability}>';
    return _FieldType(
      dartType: dartType,
      nullable: nullable,
      fromJson: (e) =>
          '($e as List<dynamic>).map((e) => ${item.fromJsonNullable('e')}).toList()',
      toJson: (e) => '$e.map((e) => ${item.toJsonNullable('e')}).toList()',
    );
  }

  if (type == 'object' && schema.containsKey('additionalProperties')) {
    final value = _resolveType(
      schema['additionalProperties'] as Map<String, dynamic>,
    );
    final dartType = 'Map<String, ${value.dartTypeWithNullability}>';
    return _FieldType(
      dartType: dartType,
      nullable: nullable,
      fromJson: (e) =>
          '($e as Map<String, dynamic>).map((k, v) => MapEntry(k, ${value.fromJsonNullable('v')}))',
      toJson: (e) =>
          '$e.map((k, v) => MapEntry(k, ${value.toJsonNullable('v')}))',
    );
  }

  switch (type) {
    case 'string':
      if (schema['format'] == 'date-time') {
        return _FieldType(
          dartType: 'DateTime',
          nullable: nullable,
          fromJson: (e) => 'DateTime.parse($e as String)',
          toJson: (e) => '$e.toIso8601String()',
        );
      }
      return _FieldType(
        dartType: 'String',
        nullable: nullable,
        fromJson: (e) => '$e as String',
        toJson: (e) => e,
      );
    case 'integer':
      return _FieldType(
        dartType: 'int',
        nullable: nullable,
        fromJson: (e) => '($e as num).toInt()',
        toJson: (e) => e,
      );
    case 'number':
      return _FieldType(
        dartType: 'double',
        nullable: nullable,
        fromJson: (e) => '($e as num).toDouble()',
        toJson: (e) => e,
      );
    case 'boolean':
      return _FieldType(
        dartType: 'bool',
        nullable: nullable,
        fromJson: (e) => '$e as bool',
        toJson: (e) => e,
      );
  }

  throw StateError('unsupported schema node: $schema');
}

String _generateType(String dartName, Map<String, dynamic> node) {
  final String body;
  if (node.containsKey('oneOf')) {
    body = _generateUnion(dartName, node);
  } else if (node['type'] == 'string' && node.containsKey('enum')) {
    body = _generateEnum(dartName, node);
  } else if (node['type'] == 'object' && node.containsKey('properties')) {
    body = _generateClass(dartName, node);
  } else {
    throw StateError('unhandled top-level schema kind for $dartName: $node');
  }

  // Exclude the node's own embedded `definitions` — those are separately
  // registered top-level types in their own right, and their refs aren't
  // this type's own refs (e.g. HistoryResponse's `definitions` embeds
  // CpuHistoryPoint's own HistoryStat dependency, which HistoryResponse
  // itself never touches directly).
  final refs = _collectRefs({...node}..remove('definitions'))..remove(dartName);
  final imports = refs.toList()..sort();
  final buf = StringBuffer(_header);
  for (final ref in imports) {
    buf.writeln("import '${_snakeFileName(ref)}.dart';");
  }
  if (imports.isNotEmpty) buf.writeln();
  buf.write(body);
  return buf.toString();
}

// Every generated file is its own Dart library with no implicit visibility
// into sibling files — walk the schema for every $ref (including the
// allOf-wrapped form) so _generateType can emit the right imports.
Set<String> _collectRefs(dynamic node) {
  final refs = <String>{};
  void walk(dynamic n) {
    if (n is Map<String, dynamic>) {
      if (n['\$ref'] is String) {
        refs.add(_refName(n['\$ref'] as String));
      }
      n.forEach((key, value) => walk(value));
    } else if (n is List) {
      for (final item in n) {
        walk(item);
      }
    }
  }

  walk(node);
  return refs;
}

const _header =
    '// GENERATED CODE - DO NOT EDIT BY HAND.\n'
    '// Regenerate: dart run tool/generate_models.dart\n\n';

String _generateEnum(String dartName, Map<String, dynamic> node) {
  final values = (node['enum'] as List).cast<String>();
  final buf = StringBuffer();
  buf.writeln('enum $dartName {');
  for (final v in values) {
    buf.writeln('  ${_enumMemberName(v)}._(\'$v\'),');
  }
  buf.writeln('  ;');
  buf.writeln();
  buf.writeln('  final String wireValue;');
  buf.writeln('  const $dartName._(this.wireValue);');
  buf.writeln();
  buf.writeln('  static $dartName fromJson(dynamic json) {');
  buf.writeln('    final value = json as String;');
  buf.writeln('    return $dartName.values.firstWhere(');
  buf.writeln('      (v) => v.wireValue == value,');
  buf.writeln(
    "      orElse: () => throw FormatException('Unknown $dartName: \$value'),",
  );
  buf.writeln('    );');
  buf.writeln('  }');
  buf.writeln();
  buf.writeln('  String toJson() => wireValue;');
  buf.writeln('}');
  return buf.toString();
}

String _enumMemberName(String wireValue) {
  final parts = wireValue.split('_');
  final first = parts.first.toLowerCase();
  final rest = parts
      .skip(1)
      .map(
        (p) =>
            p.isEmpty ? '' : p[0].toUpperCase() + p.substring(1).toLowerCase(),
      );
  return ([first, ...rest]).join();
}

class _Field {
  final String jsonKey;
  final String dartFieldName;
  final _FieldType type;
  final bool required;

  _Field(this.jsonKey, this.dartFieldName, this.type, this.required);

  bool get fieldNullable => type.nullable || !required;
}

String _toCamel(String snake) {
  final parts = snake.split('_');
  return parts.first +
      parts
          .skip(1)
          .map((p) => p.isEmpty ? '' : p[0].toUpperCase() + p.substring(1))
          .join();
}

List<_Field> _fields(Map<String, dynamic> node) {
  final props = (node['properties'] as Map<String, dynamic>? ?? {});
  final required = (node['required'] as List?)?.cast<String>().toSet() ?? {};
  final keys = props.keys.toList()..sort();
  return keys.map((key) {
    final schema = props[key] as Map<String, dynamic>;
    final type = _resolveType(schema);
    return _Field(key, _toCamel(key), type, required.contains(key));
  }).toList();
}

String _generateClass(String dartName, Map<String, dynamic> node) {
  final fields = _fields(node);
  final buf = StringBuffer();
  buf.writeln('final class $dartName {');
  for (final f in fields) {
    buf.writeln(
      '  final ${f.fieldNullable ? '${f.type.dartType}?' : f.type.dartType} ${f.dartFieldName};',
    );
  }
  buf.writeln();
  if (fields.isEmpty) {
    buf.writeln('  const $dartName();');
  } else {
    buf.writeln('  const $dartName({');
    for (final f in fields) {
      final requiredKw = f.fieldNullable ? '' : 'required ';
      buf.writeln('    ${requiredKw}this.${f.dartFieldName},');
    }
    buf.writeln('  });');
  }
  buf.writeln();
  buf.writeln('  static $dartName fromJson(dynamic json) {');
  if (fields.isEmpty) {
    buf.writeln('    return $dartName();');
  } else {
    buf.writeln('    final map = json as Map<String, dynamic>;');
    buf.writeln('    return $dartName(');
    for (final f in fields) {
      final access = "map['${f.jsonKey}']";
      final expr = f.fieldNullable
          ? f.type.fromJsonNullable(access)
          : f.type.fromJson(access);
      buf.writeln('      ${f.dartFieldName}: $expr,');
    }
    buf.writeln('    );');
  }
  buf.writeln('  }');
  buf.writeln();
  buf.writeln('  Map<String, dynamic> toJson() => {');
  for (final f in fields) {
    final expr = f.fieldNullable
        ? f.type.toJsonNullable(f.dartFieldName)
        : f.type.toJson(f.dartFieldName);
    buf.writeln("    '${f.jsonKey}': $expr,");
  }
  buf.writeln('  };');
  buf.writeln('}');
  return buf.toString();
}

String _generateUnion(String dartName, Map<String, dynamic> node) {
  final branches = (node['oneOf'] as List).cast<Map<String, dynamic>>();

  // The discriminator is whichever property carries a single-value enum;
  // every branch of every oneOf in this codebase uses the same property
  // name for it (confirmed by reading every schema before writing this).
  String? discriminatorKey;
  for (final branch in branches) {
    final props = branch['properties'] as Map<String, dynamic>;
    for (final entry in props.entries) {
      final schema = entry.value as Map<String, dynamic>;
      final enumValues = schema['enum'];
      if (enumValues is List && enumValues.length == 1) {
        discriminatorKey = entry.key;
        break;
      }
    }
    if (discriminatorKey != null) break;
  }
  if (discriminatorKey == null) {
    throw StateError('no discriminator found for union $dartName');
  }

  final buf = StringBuffer();
  buf.writeln('sealed class $dartName {');
  buf.writeln('  const $dartName();');
  buf.writeln();
  buf.writeln('  static $dartName fromJson(dynamic json) {');
  buf.writeln('    final map = json as Map<String, dynamic>;');
  buf.writeln("    final tag = map['$discriminatorKey'] as String;");
  buf.writeln('    switch (tag) {');

  final variantNames = <String>[];
  for (final branch in branches) {
    final props = branch['properties'] as Map<String, dynamic>;
    final tagValue = (props[discriminatorKey]!['enum'] as List).first as String;
    final variantName = '$dartName${_pascalFromScreamingSnake(tagValue)}';
    variantNames.add(variantName);
    buf.writeln("      case '$tagValue':");
    buf.writeln('        return $variantName.fromJson(map);');
  }
  buf.writeln('      default:');
  buf.writeln("        throw FormatException('Unknown $dartName tag: \$tag');");
  buf.writeln('    }');
  buf.writeln('  }');
  buf.writeln();
  buf.writeln('  Map<String, dynamic> toJson();');
  buf.writeln('}');
  buf.writeln();

  for (var i = 0; i < branches.length; i++) {
    final branch = branches[i];
    final variantName = variantNames[i];
    final fields = _fields(branch)
        .where((f) => f.jsonKey != discriminatorKey)
        .toList();
    final tagValue =
        (branch['properties']
                as Map<String, dynamic>)[discriminatorKey]!['enum'][0]
            as String;

    buf.writeln('final class $variantName extends $dartName {');
    for (final f in fields) {
      buf.writeln(
        '  final ${f.fieldNullable ? '${f.type.dartType}?' : f.type.dartType} ${f.dartFieldName};',
      );
    }
    if (fields.isEmpty) {
      buf.writeln('  const $variantName();');
    } else {
      buf.writeln('  const $variantName({');
      for (final f in fields) {
        final requiredKw = f.fieldNullable ? '' : 'required ';
        buf.writeln('    ${requiredKw}this.${f.dartFieldName},');
      }
      buf.writeln('  });');
    }
    buf.writeln();
    buf.writeln('  static $variantName fromJson(Map<String, dynamic> map) {');
    if (fields.isEmpty) {
      buf.writeln('    return $variantName();');
    } else {
      buf.writeln('    return $variantName(');
      for (final f in fields) {
        final access = "map['${f.jsonKey}']";
        final expr = f.fieldNullable
            ? f.type.fromJsonNullable(access)
            : f.type.fromJson(access);
        buf.writeln('      ${f.dartFieldName}: $expr,');
      }
      buf.writeln('    );');
    }
    buf.writeln('  }');
    buf.writeln();
    buf.writeln('  @override');
    buf.writeln('  Map<String, dynamic> toJson() => {');
    buf.writeln("    '$discriminatorKey': '$tagValue',");
    for (final f in fields) {
      final expr = f.fieldNullable
          ? f.type.toJsonNullable(f.dartFieldName)
          : f.type.toJson(f.dartFieldName);
      buf.writeln("    '${f.jsonKey}': $expr,");
    }
    buf.writeln('  };');
    buf.writeln('}');
    buf.writeln();
  }

  return buf.toString();
}
