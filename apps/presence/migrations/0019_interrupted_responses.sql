-- Solo conserva el fragmento recibido cuando una respuesta termina interrumpida.
ALTER TABLE presence_history ADD COLUMN interruption TEXT
    CHECK (interruption IS NULL OR (
        interruption IN ('cancelled','failed') AND speaker='assistant' AND provider_turn_id IS NOT NULL
    ));
CREATE UNIQUE INDEX presence_interrupted_without_item
    ON presence_history(conversation_id,provider_turn_id)
    WHERE interruption IS NOT NULL AND provider_item_id IS NULL;
