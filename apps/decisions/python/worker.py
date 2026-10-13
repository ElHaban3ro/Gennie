"""Proceso local de decisiones. IPC JSON; no servidores ni órdenes shell del modelo."""
import contextlib
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import time

if "DomainAdapter" not in globals():
    exec(Path(__file__).with_name('domain.py').read_text(), globals())
if "discover_applications" not in globals():
    exec(Path(__file__).with_name('applications.py').read_text(), globals())

CATALOG = {
    "open_browser": "Abrir el navegador predeterminado.",
    "open_safari": "Abrir Safari.",
    "open_chrome": "Abrir Google Chrome.",
    "open_discord": "Abrir Discord.",
    "open_files": "Abrir Finder.",
    "open_app_menu": "Abrir el menú de aplicaciones.",
    "open_spotify": "Abrir Spotify.",
    "open_music": "Abrir Música de Apple.",
    "pause_music": "Pausar la reproducción musical.",
    "play_music": "Reproducir o reanudar la música.",
    "next_track": "Pasar a la canción siguiente.",
    "previous_track": "Volver a la canción anterior.",
}
BUNDLES = {
    "open_safari": "com.apple.Safari", "open_chrome": "com.google.Chrome",
    "open_discord": "com.hnc.Discord", "open_files": "com.apple.finder",
    "open_app_menu": "com.apple.launchpad.launcher", "open_spotify": "com.spotify.client",
    "open_music": "com.apple.Music",
}
PLAYERS = {"com.spotify.client": "Spotify", "com.apple.Music": "Music"}
MUSIC_ACTIONS = ('pause_music','play_music','next_track','previous_track')
ROUTES = {
    "incomplete": "Petición empezada pero faltan palabras: esperar más voz.",
    "ambiguous": "Hay una orden pero su significado o destino no se resuelve: pedir aclaración.",
    "no_order": "No ordena actuar, es una pregunta, cita, mención o petición negada: no ejecutar.",
    "god_call": "Orden completa que pide una operación fuera del catálogo local.",
    "complete": "Una única orden afirmativa, completa y resoluble dentro del catálogo.",
}
ACTION_KEYS = {action: action for action in CATALOG}
ACTION_KEYS["god_call"] = "god_call"
ROUTE_KEYS = {route: route for route in ROUTES}
OPERATION_OPTIONS = {
    'open_app': 'Abrir o mostrar una aplicación.',
    **{action: CATALOG[action] for action in MUSIC_ACTIONS},
    'no_order': 'No hay orden: negación, cita, descripción o pregunta.',
    'incomplete': 'Faltan palabras para completar la petición.',
    'god_call': 'Una operación diferente, fuera del catálogo.',
    'ambiguous': 'No se puede determinar la operación.',
}


def run(arguments, timeout=8):
    return subprocess.run(arguments, capture_output=True, text=True, check=True, timeout=timeout).stdout.strip()


def workspace(timeout=8):
    script = '''ObjC.import('AppKit');
let w=$.NSWorkspace.sharedWorkspace;
let installed={};
for (let id of %s) {let u=w.URLForApplicationWithBundleIdentifier(id);installed[id]=!u.isNil();}
let browser=w.URLForApplicationToOpenURL($.NSURL.URLWithString('https://example.com'));
let bid=browser.isNil()?null:ObjC.unwrap($.NSBundle.bundleWithURL(browser).bundleIdentifier);
JSON.stringify({installed:installed,running:ObjC.unwrap(w.runningApplications).map(a=>ObjC.unwrap(a.bundleIdentifier)),browser:bid,frontmost:ObjC.unwrap(w.frontmostApplication.bundleIdentifier),frontmost_pid:w.frontmostApplication.processIdentifier});
''' % json.dumps(list(BUNDLES.values()))
    observed = json.loads(run(["/usr/bin/osascript", "-l", "JavaScript", "-e", script], timeout=timeout))
    # JXA puede omitir valores undefined durante un cambio de aplicación.
    # Ausencia de identidad no demuestra que el destino esté en primer plano.
    observed.setdefault('frontmost', None)
    observed.setdefault('frontmost_pid', None)
    return observed


def player_snapshot(bundle):
    app = PLAYERS[bundle]
    track = 'id of current track as text' if app == "Spotify" else 'persistent ID of current track as text'
    script = f'''tell application "{app}"
set s to player state as text
set t to ""
try
set t to {track}
end try
return s & "|" & t
end tell'''
    state, track = run(["/usr/bin/osascript", "-e", script]).split("|", 1)
    return {"state": state.lower(), "track": track}


def system_context(read_players=True):
    ctx = workspace()
    ctx['applications'] = discover_applications()
    ctx["players"] = {}
    ctx['_players_loaded'] = read_players
    if read_players:
        populate_players(ctx)
    return ctx


def populate_players(ctx):
    for bundle in PLAYERS:
        if bundle in ctx["running"]:
            try:
                ctx["players"][bundle] = player_snapshot(bundle)
            except (subprocess.SubprocessError, ValueError):
                # No inferir estado musical cuando el sistema no permite leerlo.
                ctx["players"][bundle] = None
    ctx['_players_loaded'] = True


def application_intent(agent, request, ctx):
    adapter = getattr(agent,'domain_adapter',None)
    if adapter is not None and hasattr(adapter,'original_tensors'):
        # Las adaptaciones antiguas tienen clases fijas; las apps nuevas usan Laya base.
        del agent.domain_adapter
        try:
            agent.model.load_state_dict(adapter.original_tensors,strict=False)
            return infer_application_intent(agent,request,ctx)
        finally:
            agent.model.load_state_dict(adapter.adapted_tensors,strict=False)
            agent.domain_adapter = adapter
    if adapter is not None and getattr(adapter,'metadata',{}).get('trained_encoder'):
        # No reutilizar un encoder especializado sin conservar su estado base.
        return None
    return infer_application_intent(agent,request,ctx)


def infer_application_intent(agent, request, ctx):
    # Repetir la misma orden tras una aclaración no añade contexto semántico.
    # Evitar que nuestra propia duda contamine otra vez la clasificación.
    if request.get('pending') and normalized_name(request['pending']) == normalized_name(request['text']):
        request = {**request, 'pending': None, 'pending_action': None}
    applications = ctx.get('applications', {})
    matches = matching_applications(request['text'], applications)
    explicit_target = bool(matches)
    if not matches and request.get('last_action') == 'open_app' and request.get('last_target') in applications:
        bundle = request['last_target']
        matches = [(bundle,applications[bundle])]
    if not matches:
        return None
    if len(matches)!=1:
        return {'route':'ambiguous','action':None,'target':None,'probability':None,'requested_action':'open_app','availability':None}
    bundle, app = matches[0]
    state = {'context':{'last_verified_action':request.get('last_action'),'pending_action':request.get('pending_action'),
                        'pending_utterance':request.get('pending')},'utterance':request['text']}
    if request.get('last_action')=='open_app' and request.get('last_target')==bundle:
        state['context']['last_verified_application'] = app['name']
    state = json.loads(json.dumps(state,ensure_ascii=False,sort_keys=True))
    options = {'open_app':'Abrir '+app['name']+' ahora.', 'god_call':'Otra operación, no abrir la aplicación.',
               'no_order':'No hay orden: negación, cita o descripción.','incomplete':'Faltan palabras.','ambiguous':'No queda claro.'}
    key,score = choice(agent,state,'application_request',options,'¿Qué pide el usuario?')
    # Los nulls de contexto alteraban la clasificación de frases completas.
    # La intención simple usa solo la frase; las referencias conservan su contexto real.
    contextual = bool(request.get('pending')) or not explicit_target
    speech_state = state if contextual else {'utterance':request['text']}
    # El ASR añade mayúscula/punto final sin cambiar el acto de habla.
    # Conservar preguntas, comillas y puntos suspensivos; nunca quitar palabras.
    speech_text = request['text'].strip()
    if speech_text.endswith('.') and not speech_text.endswith('..'):
        speech_text = speech_text[:-1]
    speech_text = speech_text[:1].upper()+speech_text[1:]
    speech_options = {'command':'Una orden afirmativa para actuar ahora.','no_order':'Una negación, cita, descripción o pregunta; no actuar.',
                      'incomplete':'Frase incompleta: faltan palabras.'}
    speech,certainty = choice(agent,speech_state,'speech_act',
        speech_options,'Clasifica la última frase del usuario.')
    speech_threshold = .9 if contextual else .95
    if speech=='command' and certainty<speech_threshold and speech_text!=request['text']:
        normalized_speech,normalized_certainty = choice(agent,{**speech_state,'utterance':speech_text},'speech_act',
            speech_options,'Clasifica la última frase del usuario.')
        if normalized_speech=='command' and normalized_certainty>certainty:
            certainty = normalized_certainty
    if key=='open_app' and score>=.9 and speech=='command' and certainty>=speech_threshold:
        return {'route':'complete','action':'open_app','target':bundle,'probability':min(score,certainty),'requested_action':'open_app',
                'availability':{'status':'available','reason':None}}
    route = key if key!='open_app' and score>=.9 else 'ambiguous'
    return {'route':route,'action':None,'target':None,'probability':min(score,certainty) if route=='ambiguous' else score,'requested_action':'open_app' if route=='ambiguous' else None,'availability':None}


def music_target(ctx):
    readable = {key: value for key, value in ctx["players"].items() if value is not None}
    playing = [key for key, value in readable.items() if value["state"] == "playing"]
    if len(playing) == 1:
        return playing[0]
    if len(ctx["players"]) == 1 and len(readable) == 1:
        return next(iter(readable))
    return None


def available_options(ctx):
    options = {}
    for action in CATALOG:
        if action in BUNDLES:
            if ctx["installed"].get(BUNDLES[action]):
                options[action] = CATALOG[action]
        elif action == "open_browser":
            if ctx.get("browser"):
                options[action] = CATALOG[action]
        elif ctx["players"]:
            options[action] = CATALOG[action]
    options["god_call"] = "La operación no está disponible en este catálogo local."
    return options


def choice(agent, state, name, criteria, instructions):
    if hasattr(agent, "tok"):
        # Rechazar truncamiento de la frase: podría perder una negación/corrección final.
        serialized = json.dumps(state, ensure_ascii=False)
        if len(agent.tok(serialized, add_special_tokens=False)["input_ids"]) > agent.cfg.get("max_len", 1024) - agent.cfg.get("head_max_len", 192) - 4:
            raise ValueError("El contexto excede el presupuesto de tokens")
    response = agent.predict(state, {name: {"type": "choice", "criteria": criteria, "instructions": instructions}})
    answer = response["answers"][name]
    key = answer["choice"]
    probabilities = answer["probabilities"]
    if key not in criteria or set(probabilities) != set(criteria):
        raise ValueError("Respuesta fuera de opciones")
    values = list(probabilities.values())
    if any(isinstance(v, bool) or not isinstance(v, (float, int)) or not math.isfinite(v) or not 0 <= v <= 1 for v in values):
        raise ValueError("Probabilidades inválidas")
    if abs(sum(values) - 1) > .01 or probabilities[key] < max(values):
        raise ValueError("Distribución inválida")
    return key, probabilities[key]


def infer_intent(agent, request):
    if hasattr(agent, 'domain_adapter'):
        intent = agent.domain_adapter.infer([request])[0]
        if intent['route']=='complete' and hasattr(agent.domain_adapter,'check_semantics') and not agent.domain_adapter.check_semantics(request,intent['action']):
            return {'route':'ambiguous','action':None,'requested_action':intent['action'],'probability':intent['probability']}
        return intent
    # Preguntas semánticas independientes de la instalación y de patrones de frases.
    state = {'utterance': request['text'], 'context': {
        'last_verified_action': request.get('last_action'),
        'pending_utterance': request.get('pending'),
        'pending_action': request.get('pending_action'),
    }}
    state = json.loads(json.dumps(state, ensure_ascii=False, sort_keys=True))
    route_key, probability = choice(agent, state, 'route', {ROUTE_KEYS[k]: v for k, v in ROUTES.items()},
        'Interpreta la intención final. Distingue una orden completa de una incompleta, negada o citada. '
        'Resuelve la última corrección y las referencias con la acción comprobada o la petición pendiente.')
    route = next(key for key, value in ROUTE_KEYS.items() if value == route_key)
    if route != 'complete':
        # Una elección incierta no demuestra que falten palabras o que no haya orden.
        return {'route': route if probability >= .9 else 'ambiguous', 'action': None, 'probability': probability}
    action_key, score = choice(agent, state, 'action', {ACTION_KEYS[k]: v for k, v in CATALOG.items()} | {'god_call': 'Operación fuera de este catálogo'},
        'Qué operación pide la orden completa, independientemente de si la aplicación está instalada. '
        'Una búsqueda dentro de una aplicación no equivale a abrirla. Aplica la última corrección clara.')
    action = next(key for key, value in ACTION_KEYS.items() if value == action_key)
    if action == 'god_call':
        return {'route': 'god_call', 'action': None, 'probability': min(probability, score)}
    # Son distribuciones distintas: cinco rutas frente a trece operaciones.
    # La operación conserva el corte estricto; la ruta requiere apoyo mayoritario.
    if probability < .7 or score < .9:
        return {'route': 'ambiguous', 'action': None, 'requested_action': action, 'probability': min(probability, score)}
    return {'route': 'complete', 'action': action, 'probability': min(probability, score)}


def infer_operation(agent, request):
    if hasattr(agent, 'domain_adapter'):
        intent = infer_intent(agent, request)
        if intent['route']=='complete' and intent['action'] in {*BUNDLES, 'open_browser'}:
            intent = {**intent, 'action':'open_app'}
        return intent
    # La operación se decide antes de buscar nombres de apps: «música» también
    # es un nombre común, y una app mencionada no autoriza su apertura.
    state = {'utterance':request['text']}
    context = {key:value for key,value in {
        'last_verified_action':request.get('last_action'),
        'pending_utterance':request.get('pending'),
        'pending_action':request.get('pending_action'),
    }.items() if value is not None}
    if request.get('pending') and normalized_name(request['pending']) == normalized_name(request['text']):
        context.pop('pending_utterance',None)
        context.pop('pending_action',None)
    operation,score = choice(agent,state,'operation',OPERATION_OPTIONS,
        'Selecciona la operación que pide el usuario, antes de elegir una aplicación. '
        'Una mención no es una orden y abrir un reproductor no equivale a controlar la reproducción.')
    # Una orden nueva y clara prevalece sobre la acción anterior. Solo recurrir
    # al contexto si la frase sola no ha determinado una operación ejecutable.
    if context and operation in ('incomplete','ambiguous'):
        operation,score = choice(agent,{**state,'context':context},'operation',OPERATION_OPTIONS,
            'Resuelve la referencia o aclaración con el contexto. La última petición del usuario prevalece sobre la acción anterior.')
    executable = operation=='open_app' or operation in MUSIC_ACTIONS
    if score < (.95 if executable else .9):
        return {'route':'ambiguous','action':None,'probability':score,
                'requested_action':operation if executable else None}
    return {'route':'complete' if executable else operation,
            'action':operation if executable else None,'probability':score}


def player_reference(text, players):
    matches = matching_applications(text,players)
    if len(matches)>1:
        return None,True
    if not matches:
        return None,False
    bundle = matches[0][0]
    if bundle=='com.apple.Music':
        # Music y sus traducciones son nombres comunes. En una orden musical
        # solo fijan la app como respuesta de destino o con un calificador.
        # La apertura usa el inventario completo y no pasa por esta regla.
        name = normalized_name(text)
        aliases = {normalized_name(alias) for alias in players[bundle]['aliases']}
        qualified = 'apple' in name.split() or any(' '+prefix+' '+alias+' ' in ' '+name+' '
                        for alias in aliases for prefix in ('en','in','on','app','aplicacion'))
        if name not in aliases and not qualified:
            return None,False
    return bundle,False


def execution_veto(text):
    # Solo veta efectos: no elige acciones ni autoriza frases por coincidencia.
    # La negación/cita/tiempo declarado prevalece sobre una confianza del modelo.
    normalized = ' '+normalized_name(text)+' '
    words = set(normalized.split())
    if words.intersection({'no','nunca','jamas','not','never','dont','cannot'}) or any(
            contraction in normalized for contraction in (' don t ',' can t ')):
        return 'ambiguous' if words.intersection({'mejor','instead','actually'}) else 'no_order'
    if any(mark in text for mark in ('¿','?','«','»','“','”','"')):
        return 'no_order'
    if words.intersection({'ayer','anoche','manana','luego','despues','yesterday','tomorrow','later'}):
        return 'ambiguous'
    return None


def bind_intent(intent, agent, request, ctx):
    result = dict(intent, target=None, requested_action=intent.get('requested_action') or intent.get('action'), availability=None)
    if intent['route'] != 'complete':
        return result
    action = intent['action']
    if action == 'open_app':
        target = intent.get('target')
        available = target in ctx.get('applications',{})
        result.update(target=target if available else None,availability={'status':'available' if available else 'unavailable','reason':None if available else 'application_missing'})
        return result
    if action not in CATALOG:
        raise ValueError('Acción desconocida')
    if action in BUNDLES or action == 'open_browser':
        target = ctx.get('browser') if action == 'open_browser' else BUNDLES[action]
        available = bool(target) and (action == 'open_browser' or ctx['installed'].get(target, False))
        result.update(target=target if available else None, availability={'status': 'available' if available else 'unavailable', 'reason': None if available else 'application_missing'})
        return result
    target = intent.get('target') if intent.get('target') in PLAYERS else None
    # Nombres de entidades son datos, no una gramática de órdenes.
    text = request['text'].casefold()
    spotify = 'spotify' in text
    apple = 'apple music' in text or 'música de apple' in text
    if spotify != apple:
        target = 'com.spotify.client' if spotify else 'com.apple.Music'
    elif spotify and apple:
        key, score = choice(agent, {'utterance': request['text'], 'pending': request.get('pending')}, 'player',
            {'A': 'La intención final elige Spotify.', 'B': 'La intención final elige Música de Apple.', 'C': 'No se puede determinar el reproductor.'},
            'Selecciona el reproductor de la última corrección clara; no ejecutes ambos.')
        target = {'A': 'com.spotify.client', 'B': 'com.apple.Music'}.get(key) if score >= .8 else None
    elif target is None:
        players = {bundle:app for bundle,app in ctx.get('applications',{}).items() if bundle in PLAYERS}
        target,unresolved = player_reference(request['text'],players)
        if unresolved:
            result.update(route='ambiguous',action=None,availability={'status':'ambiguous','reason':'choose_player'})
            return result
        if target is None and request.get('pending_action') in MUSIC_ACTIONS and request.get('pending'):
            # La aclaración no debe cambiar al reproductor que casualmente esté sonando.
            pending_players = dict(players)
            for bundle,name in PLAYERS.items():
                pending_players.setdefault(bundle,{'aliases':[name,'Apple Music'] if bundle=='com.apple.Music' else [name]})
            target,unresolved = player_reference(request['pending'],pending_players)
            if unresolved:
                result.update(route='ambiguous',action=None,availability={'status':'ambiguous','reason':'choose_player'})
                return result
        if target is None and request.get('last_action') in MUSIC_ACTIONS and request.get('last_target') in PLAYERS:
            target = request['last_target']
        if target is None:
            target = music_target(ctx)
    if target is None:
        if ctx['players']:
            result.update(route='ambiguous', action=None, availability={'status': 'ambiguous', 'reason': 'choose_player'})
        else:
            result.update(availability={'status': 'unavailable', 'reason': 'no_player'})
    elif target not in ctx['players']:
        result.update(availability={'status': 'unavailable', 'reason': 'player_not_running'})
    elif ctx['players'][target] is None:
        result.update(availability={'status': 'unavailable', 'reason': 'player_unreadable'})
    else:
        result.update(target=target, availability={'status': 'available', 'reason': None})
    return result


def decide(agent, request, ctx):
    veto = execution_veto(request['text'])
    if veto is not None:
        return bind_intent({'route':veto,'action':None,'probability':None},agent,request,ctx)
    operation = infer_operation(agent,request)
    intent = operation
    if operation['route']=='complete' and operation['action']=='open_app':
        intent = application_intent(agent,request,ctx)
        intent = intent if intent is not None else infer_intent(agent, request)
        if intent.get('action') in MUSIC_ACTIONS:
            # Un desacuerdo entre clasificadores no autoriza otra operación.
            intent = {'route':'ambiguous','action':None,'requested_action':intent['action'],
                      'probability':min(operation['probability'],intent['probability'])}
    if intent.get('action') in ('pause_music','play_music','next_track','previous_track') and not ctx.get('_players_loaded',True):
        populate_players(ctx)
    return bind_intent(intent, agent, request, ctx)


def failure_category(error):
    # Códigos fijos: no exponer stderr, argumentos ni contenido del usuario.
    if isinstance(error, subprocess.TimeoutExpired):
        return 'os_timeout'
    if isinstance(error, subprocess.CalledProcessError):
        return 'os_command_failed'
    if isinstance(error, OSError):
        return 'os_unavailable'
    if isinstance(error, (ValueError, KeyError, TypeError)):
        return 'invalid_response'
    return 'internal_error'


def open_application(arguments, target):
    diagnostic = {'stage':'application_verification'}
    try:
        run(arguments)
    except OSError as error:
        # No se pudo crear el proceso: no se envió la apertura.
        return {'verified':False, 'evidence':json.dumps({'stage':'application_open', 'reason':failure_category(error)})}
    except subprocess.SubprocessError as error:
        # open puede haber entregado la petición antes de fallar o agotar su espera.
        # Comprobar el efecto igualmente, sin reenviar la apertura.
        diagnostic['command_warning'] = failure_category(error)
    deadline = time.monotonic() + 4.0
    observed_once = False
    for _ in range(12):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        try:
            observed = workspace(timeout=min(1.0, remaining))
            running, frontmost = observed['running'], observed['frontmost']
            if not isinstance(running, list) or (frontmost is not None and not isinstance(frontmost, str)):
                raise ValueError('Estado de aplicaciones inválido')
            observed_once = True
            if target in running and frontmost == target:
                evidence = {'running_bundle':target}
                if 'command_warning' in diagnostic or 'observation_failure' in diagnostic:
                    evidence['verification'] = diagnostic
                return {'verified':True, 'evidence':json.dumps(evidence)}
        except (OSError, subprocess.SubprocessError, ValueError, KeyError, TypeError) as error:
            diagnostic['observation_failure'] = failure_category(error)
        remaining = deadline - time.monotonic()
        if remaining > 0:
            time.sleep(min(.15, remaining))
    diagnostic['reason'] = 'application_not_observed' if observed_once else 'observation_unavailable'
    return {'verified':False, 'uncertain':True, 'evidence':json.dumps(diagnostic)}


def execute(request):
    action, target = request["action"], request["target"]
    musical = action in ('pause_music','play_music','next_track','previous_track')
    ctx = system_context(read_players=musical)
    if action == 'open_app':
        app = ctx.get('applications',{}).get(target)
        if app is None:
            return {'verified':False,'evidence':'context_changed'}
        return open_application(['/usr/bin/open','-a',app['path']], target)
    # La disponibilidad se vuelve a comprobar justo antes del efecto.
    if action not in available_options(ctx) or action == "god_call":
        return {"verified": False, "evidence": "unavailable"}
    if action in BUNDLES or action == "open_browser":
        expected = ctx.get("browser") if action == "open_browser" else BUNDLES[action]
        if target != expected:
            return {"verified": False, "evidence": "context_changed"}
        return open_application(["/usr/bin/open", "-b", target], target)
    if target not in PLAYERS or target not in ctx['players'] or ctx['players'][target] is None:
        return {"verified": False, "evidence": "context_changed"}
    before = ctx["players"][target]
    if action in ("next_track", "previous_track") and not before["track"]:
        return {"verified": False, "evidence": "track_unreadable"}
    command = {"pause_music": "pause", "play_music": "play", "next_track": "next track", "previous_track": "previous track"}[action]
    run(["/usr/bin/osascript", "-e", f'tell application "{PLAYERS[target]}" to {command}'])
    for _ in range(12):
        after = player_snapshot(target)
        verified = ((action == "pause_music" and after["state"] in ("paused", "stopped"))
                    or (action == "play_music" and after["state"] == "playing")
                    or (action in ("next_track", "previous_track") and bool(after["track"]) and after["track"] != before["track"]))
        if verified:
            return {"verified": True, "evidence": json.dumps({"player": target, "before": before, "after": after})}
        time.sleep(.15)
    return {"verified": False, "evidence": "effect_not_observed"}


def load_local_agent(model):
    import torch
    # Evitar saturar CPU durante la carga mientras Nemotron se prepara.
    torch.set_num_threads(2)
    import laya
    from transformers.initialization import no_init_weights
    model = Path(model)
    if not model.is_dir() or not (model/'encoder/config.json').is_file():
        raise ValueError('Checkpoint y encoder locales requeridos')
    # El checkpoint completo sustituye todos los parámetros con strict=True en Laya.
    # Omitir su inicialización aleatoria; conservar buffers, dtype y carga originales.
    # El helper es serial: el contexto temporal no coincide con otra inferencia.
    with no_init_weights():
        return laya.load(str(model), device='mps')


def main():
    agent = None
    issued = None
    model = Path(sys.argv[1])
    def load_agent():
        loaded = load_local_agent(model)
        if len(sys.argv)>2 and sys.argv[2]:
            loaded.domain_adapter = DomainAdapter(loaded,sys.argv[2],file_hash(model/'model.safetensors'))
        # Preparar el primer forward de GPU sin petición, contexto OS ni plan ejecutable.
        choice(loaded,'No hay ninguna petición de usuario.','warmup',{'idle':'Sin petición.','request':'Hay una petición.'},'Clasifica el estado de inicio.')
        return loaded
    for line in sys.stdin:
        # Todo mensaje consume el plan anterior, incluso si falla su validación.
        # Un error no puede dejar una autorización reutilizable para otro execute.
        previous_plan, issued = issued, None
        try:
            if len(line) > 16384:
                raise ValueError("Entrada demasiado larga")
            request = json.loads(line)
            # Las librerías no deben contaminar stdout, reservado para IPC.
            with contextlib.redirect_stdout(sys.stderr):
                if request['op'] == 'load':
                    issued = None
                    if agent is None:
                        agent = load_agent()
                    result = {'ready':True}
                elif request["op"] == "decide":
                    ctx = request.get("test_context") or system_context(read_players=False)
                    if agent is None:
                        agent = load_agent()
                    result = decide(agent, request, ctx)
                    issued = {'action': result['action'], 'target': result['target'], 'request': request} if result['route'] == 'complete' and result.get('target') else None
                elif request["op"] == "execute":
                    # No aceptar un efecto sin la intención confirmada en este proceso.
                    if previous_plan is None or (request['action'], request['target']) != (previous_plan['action'], previous_plan['target']):
                        raise ValueError('Ejecución no autorizada')
                    musical = previous_plan['action'] in ('pause_music','play_music','next_track','previous_track')
                    refreshed = bind_intent({'route': 'complete', 'action': previous_plan['action'], 'target':previous_plan['target'], 'probability': 1.0}, agent, previous_plan['request'], system_context(read_players=musical))
                    result = execute(request) if refreshed['target'] == request['target'] else {'verified': False, 'evidence': 'context_changed'}
                else:
                    raise ValueError("Operación inválida")
            print(json.dumps(result, ensure_ascii=False), flush=True)
        except Exception as error:
            # No publicar texto del usuario ni excepciones de terceros en logs.
            print(json.dumps({"error": "local_worker_failed", "category":failure_category(error)}), flush=True)


if __name__ == "__main__":
    main()
