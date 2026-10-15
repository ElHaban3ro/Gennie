DELETE FROM presence_history
WHERE speaker = 'system'
  AND instr(text, char(10) || 'Origen: ') > 0
  AND (
    text LIKE 'Recuperado (%' OR
    text LIKE 'Recuerdo guardado · #%'
  );
