// Presentación y validación local del contrato de entrada; Rust/Node vuelven a validarlo.
class PluginParameter {
  PluginParameter(this.name, Object definition)
    : spec = definition is String
          ? {'type': definition}
          : Map<String, dynamic>.from(definition as Map);

  final String name;
  final Map<String, dynamic> spec;
  String get type => spec['type'] as String;
  String get label => spec['label'] as String? ?? name;
  String? get description => spec['description'] as String?;
  bool get isRequired => spec['required'] as bool? ?? true;
  List<Object> get options => spec['enum'] != null
      ? List<Object>.from(spec['enum'] as List)
      : type == 'boolean'
      ? [true, false]
      : [];
  String display(Object value) =>
      value is bool ? (value ? 'Sí' : 'No') : '$value';
  Object? parse(String text) {
    if (text.isEmpty) return null;
    return switch (type) {
      'integer' => int.tryParse(text) ?? text,
      'number' => num.tryParse(text) ?? text,
      _ => text,
    };
  }

  String reason(String code) => switch (code) {
    'required' => 'Completa $label.',
    'minLength' => 'Escribe al menos ${spec['minLength'] ?? 1} caracteres.',
    'maxLength' => 'Usa como máximo ${spec['maxLength'] ?? 1000} caracteres.',
    'minimum' => 'El mínimo es ${spec['minimum'] ?? -9007199254740991}.',
    'maximum' => 'El máximo es ${spec['maximum'] ?? 9007199254740991}.',
    'enum' => 'Elige una de las opciones de $label.',
    'type' when type == 'integer' => '$label debe ser un entero.',
    'type' when type == 'number' => '$label debe ser un número válido.',
    _ => 'Revisa $label antes de ejecutar.',
  };

  String? validate(Object? value) {
    if (value == null) return isRequired ? reason('required') : null;
    if (type == 'string') {
      if (value is! String || value.contains('\u0000')) return reason('type');
      if (value.runes.length < (spec['minLength'] as int? ?? 1)) {
        return reason('minLength');
      }
      if (value.runes.length > (spec['maxLength'] as int? ?? 1000)) {
        return reason('maxLength');
      }
    } else if (type == 'boolean') {
      if (value is! bool) return reason('type');
    } else {
      if (value is! num ||
          !value.isFinite ||
          (type == 'integer' && value is! int)) {
        return reason('type');
      }
      if (value < (spec['minimum'] as num? ?? -9007199254740991)) {
        return reason('minimum');
      }
      if (value > (spec['maximum'] as num? ?? 9007199254740991)) {
        return reason('maximum');
      }
    }
    if (spec['enum'] is List && !(spec['enum'] as List).contains(value)) {
      return reason('enum');
    }
    return null;
  }
}
