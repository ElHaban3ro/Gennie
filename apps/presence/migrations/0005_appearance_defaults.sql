-- Perfil confirmado por el usuario; conserva cualquier combinación personalizada.
UPDATE presence_appearance SET
    blur_radius=0, saturation_percent=200, brightness_percent=101,
    tint_percent=14, border_percent=0, border_width_hundredths=300,
    transition_ms=300, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')
WHERE id=1 AND blur_radius=6 AND saturation_percent=100 AND brightness_percent=100
    AND tint_percent=24 AND border_percent=33 AND border_width_hundredths=75
    AND transition_ms=120;
