#!/usr/bin/env python3
import unittest
import pilot_native_goal


class GoalEventsTest(unittest.TestCase):
    def test_goal_closure_requires_child_turn_and_matching_parent_proof(self):
        goal={'status':'active'}
        observations=[{'method':'turn/completed','threadId':'child'}]
        evidence=[{'threadId':'child','passed':True,'recipe':{'x':1}},
                  {'threadId':'parent','passed':True,'recipe':{'x':1}}]
        self.assertTrue(pilot_native_goal.may_close_executor('child','parent',goal,observations,evidence))
        self.assertFalse(pilot_native_goal.may_close_executor('child','parent',goal,[],evidence))
        evidence[1]['recipe']={'x':2}
        self.assertFalse(pilot_native_goal.may_close_executor('child','parent',goal,observations,evidence))
        self.assertFalse(pilot_native_goal.may_close_executor('child','parent',{'status':'blocked'},observations,evidence))

    def test_root_completion_does_not_hide_active_executor(self):
        report={'goal_status':'complete','verification':{'passed':True},
                'executor_goals':{'child':{'status':'accepted'}},
                'goal_updates_by_thread':{'child':{'status':'active'}},
                'agent_parents':{'child':'engineer'},
                'plan_evidence':[{'threadId':'engineer','status':'plan_recorded','proposal_count':2}],
                'recipe_evidence':[{'threadId':'child','recipe':{'x':1},'passed':True},
                                   {'threadId':'engineer','recipe':{'x':1},'passed':True}],
                'calls':[{'tool':'test_recipe','meta_thread_id':'child','arguments':{'recipe':{'x':1}},'result':{'passed':True}},
                         {'tool':'test_recipe','meta_thread_id':'engineer','arguments':{'recipe':{'x':1}},'result':{'passed':True}}],
                'codex_usage_by_thread':{'child':{'total':{'totalTokens':10}}}}
        self.assertEqual(pilot_native_goal.audit(report)['failures'],['executor_goal_incomplete'])
        report['goal_updates_by_thread']['child']['status']='complete'
        self.assertTrue(pilot_native_goal.audit(report)['passed'])
        report['recipe_evidence'][1]['recipe']={'x':2}
        self.assertEqual(pilot_native_goal.audit(report)['failures'],['recipe_attribution_incomplete','parent_verification_missing'])

    def test_executor_goal_requested_once_and_acknowledged(self):
        client=object.__new__(pilot_native_goal.Client)
        client.thread_usage={}
        client.case='simple'
        client.observations=[]
        client.executor_goals={}
        client.agent_parents={}
        client.agent_depths={}
        client.recipe_evidence=[]
        client.plan_evidence=[]
        client.goal_updates={}
        client.root_turns_started=0
        client.thread_id=None
        client.turn_started=False
        client.messages={}
        client.generation_requested=True
        sent=[]
        client.send=sent.append
        event={'method':'item/started','params':{'threadId':'root','item':{
            'type':'subAgentActivity','agentPath':'/root/web/executor','agentThreadId':'child'}}}
        client.observe(event)
        client.observe(event)
        self.assertEqual(len(sent),1)
        self.assertEqual(sent[0]['method'],'thread/goal/set')
        self.assertEqual(sent[0]['params']['threadId'],'child')
        client.observe({'id':100,'result':{'goal':{'status':'active'}}})
        self.assertEqual(client.executor_goals['child']['status'],'accepted')


if __name__ == '__main__':
    unittest.main()
