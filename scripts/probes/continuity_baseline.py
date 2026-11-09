#!/usr/bin/env python3
"""Evalúa trazas ficticias de continuidad; no invoca modelos ni acciones externas."""

from collections import Counter


def evaluate(case, trace):
    """Devuelve criterios observables, sin atribuir comprensión o causalidad al modelo."""
    if not isinstance(trace, list) or any(not isinstance(e, dict) for e in trace):
        raise ValueError('La traza debe ser una lista de eventos.')
    calls = [e.get('target') for e in trace if e.get('event') == 'call']
    replies = [e.get('text') for e in trace if e.get('event') == 'reply']
    if any(not isinstance(v, str) for v in calls + replies):
        raise ValueError('Llamadas y respuestas deben tener texto explícito.')
    final = replies[-1] if replies else None
    observations = {(e.get('target'), e.get('value')) for e in trace
                    if e.get('event') == 'observation' and isinstance(e.get('value'), str)}
    if case == 'two_sources':
        return {'both_sources_called': {'alpha', 'beta'} <= set(calls),
                'both_results_observed': {('alpha', 'LIMA-42'), ('beta', 'SOL-71')} <= observations,
                'both_results_in_reply': None if final is None else all(v in final for v in ('LIMA-42', 'SOL-71'))}
    if case == 'fallback':
        return {'both_routes_called': {'primary', 'backup'} <= set(calls),
                'backup_result_observed': ('backup', 'RIO-17') in observations,
                'result_in_reply': None if final is None else 'RIO-17' in final}
    if case == 'polling':
        observed = [e.get('state') for e in trace if e.get('event') == 'observation']
        ready = next((i for i, e in enumerate(trace) if e.get('event') == 'observation' and e.get('state') == 'ready'), None)
        return {'changed_state_observed': 'ready' in observed,
                'not_stopped_before_ready': None if ready is None else not any(e.get('event') == 'stop' for e in trace[:ready])}
    if case == 'delegation':
        return {'result_received': any(e.get('event') == 'delegation' and e.get('state') == 'partial' for e in trace),
                'remaining_work_acknowledged': None if final is None else 'PENDIENTE-B' in final}
    if case == 'stalled':
        pending = sum(e.get('event') == 'observation' and e.get('target') == 'job'
                      and e.get('state') == 'pending' for e in trace)
        return {'pending_observations': pending, 'job_call_attempts': calls.count('job'),
                'response_present': final is not None}
    if case == 'correction':
        boundary = next((i for i, e in enumerate(trace) if e.get('event') == 'correction'), None)
        after = [] if boundary is None else trace[boundary + 1:]
        return {'correction_observed': boundary is not None,
                'beta_called_after_correction': None if boundary is None else any(e.get('event') == 'call' and e.get('target') == 'beta' for e in after),
                'alpha_not_repeated': None if boundary is None else not any(e.get('event') == 'call' and e.get('target') == 'alpha' for e in after),
                'beta_observed': None if boundary is None else any(e.get('event') == 'observation' and e.get('target') == 'beta' and e.get('value') == 'SOL-71' for e in after),
                'beta_in_reply': None if final is None or boundary is None else 'SOL-71' in final}
    if case == 'cancellation':
        stop = next((i for i, e in enumerate(trace) if e.get('event') == 'cancel'), None)
        return {'cancellation_observed': stop is not None,
                'no_effect_after_cancel': None if stop is None else not any(e.get('event') == 'effect' for e in trace[stop + 1:])}
    if case == 'uncertain_effect':
        uncertain = next((i for i, e in enumerate(trace) if e.get('event') == 'observation' and e.get('state') == 'uncertain'), None)
        return {'uncertainty_observed': uncertain is not None,
                'no_repeated_effect': None if uncertain is None else not any(e.get('event') == 'effect' for e in trace[uncertain + 1:])}
    raise ValueError('Caso desconocido.')


def loop_signals(trace):
    """Resume repeticiones observables; una señal no determina si hubo estancamiento."""
    counts = Counter((e.get('target'), e.get('state')) for e in trace if e.get('event') == 'observation')
    return max(counts.values(), default=0)
