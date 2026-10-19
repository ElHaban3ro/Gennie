-- Revisión del perfil confirmado; conserva otras combinaciones personalizadas.
UPDATE presence_appearance SET
    tint_percent=5, border_width_hundredths=67,
    updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')
WHERE id=1 AND blur_radius=0 AND saturation_percent=200 AND brightness_percent=101
    AND tint_percent=14 AND border_percent=0 AND border_width_hundredths=300
    AND transition_ms=300;
