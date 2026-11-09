#!/usr/bin/env python3
"""Orquestador experimental aislado: padre, plan, ejecutores, diagnóstico y validación."""
import pilot_fixture


class PilotError(Exception):
    pass


def run(case, agent, max_attempts=4):
    """agent(role, payload) retorna un diccionario; cada llamada representa una sesión aislada."""
    if case not in pilot_fixture.CASES or type(max_attempts) is not int or not 1 <= max_attempts <= 4:
        raise PilotError('invalid_scope')
    request = pilot_fixture.CASES[case]['request']
    events = []

    def call(role, payload):
        events.append({'event':'agent_started','role':role})
        response = agent(role,payload)
        if not isinstance(response,dict):
            raise PilotError('invalid_agent_response')
        events.append({'event':'agent_finished','role':role})
        return response

    framing = call('architect',{'request':request})
    objective = framing.get('objective')
    if not isinstance(objective,str) or not objective.strip() or len(objective)>4000:
        return {'status':'invalid_objective','events':events,'attempts':[]}
    decision = call('decision',{'request':request,'objective':objective,'engineers':['web']})
    if decision.get('engineer') != 'web':
        return {'status':'routing_failed','events':events,'attempts':[]}
    plan = call('planner',{'request':request,'objective':objective,'minimum_proposals':2})
    proposals = plan.get('proposals')
    if not isinstance(proposals,list) or len(proposals)<2 or any(not isinstance(p,str) or not p.strip() for p in proposals):
        return {'status':'invalid_plan','events':events,'attempts':[]}
    attempts = []
    selected = 0
    errors = []
    diagnoses = []
    repeated = 0
    previous = None
    while len(attempts) < max_attempts and selected < len(proposals):
        result = call('executor',{'request':request,'objective':objective,'proposal':proposals[selected],
                                  'previous_errors':errors[-3:],'previous_diagnoses':diagnoses[-3:]})
        recipe = result.get('recipe')
        # La comprobación ciega es externa al veredicto del ejecutor y no concede permisos.
        observed = pilot_fixture.verify(case,recipe)
        attempts.append({'proposal_index':selected,'recipe':recipe,'verification':observed})
        if observed['passed']:
            review = call('parent_validator',{'request':request,'recipe':recipe,
                                             'observed':observed,'executor_verdict':result.get('verdict')})
            if review.get('accepted') is True:
                return {'status':'completed','events':events,'attempts':attempts,'parent_review':review}
            code = 'parent_rejected'
        else:
            code = observed['status']
        errors.append(code)
        diagnosis = call('diagnostician',{'request':request,'error':code,'previous_errors':errors[-3:]})
        text = diagnosis.get('diagnosis')
        if not isinstance(text,str) or not text.strip() or len(text)>2000:
            return {'status':'invalid_diagnosis','events':events,'attempts':attempts,'errors':errors}
        diagnoses.append(text)
        repeated = repeated+1 if code == previous else 1
        previous = code
        judgment = call('stagnation_decision',{'error':code,'previous_errors':errors[-3:],
                                               'diagnoses':diagnoses[-3:],'same_error_count':repeated})
        if type(judgment.get('stalled')) is not bool:
            return {'status':'invalid_stall_decision','events':events,'attempts':attempts,'errors':errors}
        events.append({'event':'stall_decision','stalled':judgment['stalled'],
                       'classifier_repeated':repeated >= 2,'error':code})
        if judgment['stalled']:
            selected += 1
            repeated = 0
            previous = None
    return {'status':'exhausted','events':events,'attempts':attempts,'errors':errors}
