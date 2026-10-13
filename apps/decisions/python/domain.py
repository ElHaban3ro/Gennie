"""Adaptación supervisada del encoder de Laya, sin gramática de frases en runtime."""
import hashlib
import json
from pathlib import Path

DOMAIN_ACTIONS = ['open_browser', 'open_safari', 'open_chrome', 'open_discord', 'open_files', 'open_app_menu', 'open_spotify', 'open_music', 'pause_music', 'play_music', 'next_track', 'previous_track']
DOMAIN_ROUTES = ['incomplete', 'ambiguous', 'no_order', 'god_call']
DOMAIN_LABELS = DOMAIN_ACTIONS + DOMAIN_ROUTES
DESCRIPTIONS = ['Abrir el navegador predeterminado', 'Abrir Safari', 'Abrir Chrome', 'Abrir Discord', 'Abrir Finder', 'Abrir el lanzador de aplicaciones', 'Abrir Spotify', 'Abrir Música de Apple', 'Pausar música', 'Reanudar música', 'Pasar a la siguiente canción', 'Volver a la canción anterior']


def validate_thresholds(metadata):
    thresholds = metadata.get('action_thresholds', {})
    disabled = metadata.get('disabled_actions', [])
    if not isinstance(thresholds, dict) or not isinstance(disabled, list):
        raise ValueError('Política de confianza inválida')
    if not set(thresholds).issubset(DOMAIN_ACTIONS) or not set(disabled).issubset(DOMAIN_ACTIONS):
        raise ValueError('Acción de confianza desconocida')
    for value in [metadata['threshold'], *thresholds.values()]:
        if isinstance(value, bool) or not isinstance(value, (int, float)) or not 0 <= value <= 1:
            raise ValueError('Umbral inválido')


def file_hash(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as file:
        for block in iter(lambda: file.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def semantic_text(request):
    last = request.get('last_action')
    pending = request.get('pending')
    pending_action = request.get('pending_action')
    description = lambda action: DESCRIPTIONS[DOMAIN_ACTIONS.index(action)] if action in DOMAIN_ACTIONS else 'ninguna'
    return ('Usuario: ' + request['text'] + '\nÚltima acción comprobada: ' + description(last)
            + '\nPetición pendiente: ' + (pending or 'ninguna')
            + '\nAcción pendiente: ' + description(pending_action))


def encode(agent, requests):
    import torch
    texts = [semantic_text(request) for request in requests]
    lengths = [len(agent.tok(text, add_special_tokens=True)['input_ids']) for text in texts]
    if max(lengths) > 512:
        raise ValueError('Contexto demasiado largo; no truncar correcciones')
    tokens = agent.tok(texts, padding=True, return_tensors='pt')
    tokens = {key: value.to(agent.device) for key, value in tokens.items() if key in ('input_ids', 'attention_mask')}
    with torch.inference_mode():
        hidden = agent.model.encoder(**tokens).last_hidden_state.float()
        mask = tokens['attention_mask'].unsqueeze(-1)
        pooled = (hidden * mask).sum(1) / mask.sum(1).clamp_min(1)
        features = torch.cat([hidden[:, 0], pooled], dim=1)
        return features.cpu().clone()


def make_head(dimension, classes):
    import torch
    return torch.nn.Sequential(torch.nn.Linear(dimension, 256), torch.nn.GELU(), torch.nn.Dropout(.1), torch.nn.Linear(256, classes))


class DomainAdapter:
    def __init__(self, agent, directory, base_hash=None):
        import torch
        from safetensors.torch import load_file
        directory = Path(directory)
        metadata = json.loads((directory/'manifest.json').read_text())
        if metadata['version'] not in (1, 2) or metadata['labels'] != DOMAIN_LABELS:
            raise ValueError('Adaptación incompatible')
        validate_thresholds(metadata)
        if base_hash is not None and metadata['base_sha256'] != base_hash:
            raise ValueError('La adaptación pertenece a otro checkpoint')
        self.agent, self.metadata = agent, metadata
        self.threshold = metadata['threshold']
        if not 0 <= self.threshold <= 1:
            raise ValueError('Umbral inválido')
        if metadata['version'] == 2:
            if file_hash(directory/'typed.safetensors') != metadata['typed_sha256']:
                raise ValueError('Checksum de preguntas adaptadas incorrecto')
            tensors = load_file(str(directory/'typed.safetensors'))
            names = set(agent.model.state_dict())
            if not set(tensors).issubset(names):
                raise ValueError('Pesos de adaptación desconocidos')
            self.original_tensors = {key:agent.model.state_dict()[key].detach().cpu().clone() for key in tensors}
            self.adapted_tensors = tensors
            agent.model.load_state_dict(tensors, strict=False)
            agent.model.eval()
            return
        if file_hash(directory/'head.safetensors') != metadata['head_sha256']:
            raise ValueError('Checksum de adaptación incorrecto')
        if metadata.get('trained_encoder'):
            if file_hash(directory/'encoder-last.safetensors') != metadata['encoder_sha256']:
                raise ValueError('Checksum de encoder adaptado incorrecto')
            layer = agent.model.encoder.layers[-1]
            prefix = 'encoder.layers.'+str(len(agent.model.encoder.layers)-1)+'.'
            self.original_tensors = {prefix+key:value.detach().cpu().clone() for key,value in layer.state_dict().items()}
            adapted = load_file(str(directory/'encoder-last.safetensors'))
            self.adapted_tensors = {prefix+key:value for key,value in adapted.items()}
            layer.load_state_dict(adapted)
        tensors = load_file(str(directory/'head.safetensors'))
        self.mean, self.scale = tensors.pop('feature_mean'), tensors.pop('feature_scale')
        self.head = make_head(metadata['dimension'], len(DOMAIN_LABELS))
        self.head.load_state_dict(tensors)
        self.head.eval()
        self.agent, self.metadata = agent, metadata
        self.threshold = metadata['threshold']
        if not 0 <= self.threshold <= 1:
            raise ValueError('Umbral inválido')

    def infer(self, requests):
        import torch
        if self.metadata['version'] == 2:
            from laya.common import build_sequence, collate_items
            items = []
            for request in requests:
                text = semantic_text(request)
                if len(self.agent.tok(text)['input_ids']) > 760:
                    raise ValueError('Contexto demasiado largo')
                ids,markers = build_sequence(self.agent.tok, text, self.metadata['question'], 1024, 256)
                if len(markers) != len(DOMAIN_LABELS):
                    raise ValueError('Opciones truncadas')
                items.append([{'ids': ids, 'markers': markers, 'qtype': 0}])
            data = collate_items(items, self.agent.tok.pad_token_id)
            with torch.inference_mode():
                logits,_ = self.agent.model(*(data[key].to(self.agent.device) for key in ('input_ids', 'attention_mask', 'marker_pos', 'marker_mask', 'qtype')))
            return self.route_probabilities(logits.cpu().softmax(-1))
        features = (encode(self.agent, requests) - self.mean) / self.scale
        with torch.inference_mode():
            probabilities = self.head(features).softmax(-1)
        return self.route_probabilities(probabilities)

    def route_probabilities(self, probabilities):
        results = []
        for values in probabilities:
            index = int(values.argmax())
            label, probability = DOMAIN_LABELS[index], float(values[index])
            threshold = self.metadata.get('action_thresholds', {}).get(label, self.threshold)
            if label in DOMAIN_ACTIONS and (probability < threshold or label in self.metadata.get('disabled_actions', [])):
                results.append({'route': 'ambiguous', 'action': None, 'requested_action': label, 'probability': probability})
            elif label in DOMAIN_ACTIONS:
                results.append({'route': 'complete', 'action': label, 'probability': probability})
            else:
                results.append({'route': label, 'action': None, 'probability': probability})
        return results

    def check_semantics(self, request, action):
        if self.metadata['version'] != 2:
            return True
        state = {'utterance':request['text'],'last_verified':request.get('last_action'),'pending':request.get('pending'),'pending_action':request.get('pending_action')}
        description = DESCRIPTIONS[DOMAIN_ACTIONS.index(action)]
        questions = {
            'order': {'type':'noul','instructions':'The final user utterance is an affirmative request to act now, rather than a mention, quotation, negation or description.'},
            'exact': {'type':'noul','instructions':'The final user request is complete and asks for exactly this operation now: '+description+'. A different operation or unresolved reference does not match. Resolve references using provided context.'},
        }
        try:
            self.agent.model.load_state_dict(self.original_tensors,strict=False)
            answers = self.agent.predict(state,questions)['answers']
            return all(answers[name]['noul'] >= .9 for name in questions)
        finally:
            self.agent.model.load_state_dict(self.adapted_tensors,strict=False)
