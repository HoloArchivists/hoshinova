import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { RecorderState } from '../bindings/RecorderState';
import { TaskWithStatus } from '../bindings/TaskWithStatus';
import { rejectError } from './api';
import { CreateTaskRequest } from '../bindings/CreateTaskRequest';

export const stateString = (state: RecorderState) => {
  if (typeof state === 'object' && 'Waiting' in state)
    return 'Waiting (' + state.Waiting + ')';
  else if (state === 'AlreadyProcessed') return 'Already Processed';
  else return state;
};
export const stateKey = (state: RecorderState) =>
  typeof state === 'object'
    ? (Object.keys(state) as (keyof typeof state)[])[0]
    : state;

const stateSort: ReturnType<typeof stateKey>[] = [
  'Recording',
  'Muxing',
  'Waiting',
  'Finished',
  'Idle',
  'Ended',
  'AlreadyProcessed',
  'Interrupted',
];
export const useQueryTasks = () =>
  useQuery(
    ['tasks'],
    () =>
      fetch('/api/tasks')
        .then((res) => res.json())
        .then((res) =>
          (res as TaskWithStatus[]).sort(
            (a, b) =>
              stateSort.indexOf(stateKey(a.status.state)) -
              stateSort.indexOf(stateKey(b.status.state))
          )
        ),
    {
      refetchInterval: 1000,
      keepPreviousData: true,
    }
  );

export const useMutateCreateTask = () => {
  const queryClient = useQueryClient();
  return useMutation(
    (task: CreateTaskRequest) =>
      fetch('/api/task', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(task),
      }).then(rejectError),
    {
      onSuccess: () => {
        queryClient.invalidateQueries(['tasks']);
      },
    }
  );
};
