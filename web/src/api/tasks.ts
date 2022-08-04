import { useQuery } from '@tanstack/react-query';

export type TaskWithStatus = {
  task: Task;
  status: Status;
};

export type Task = {
  title: string;
  video_id: string;
  video_picture: string;
  channel_name: string;
  channel_id: string;
  channel_picture: string;
  output_directory: string;
};

export type Status = {
  version: string;
  state: State;
  last_output: string;
  last_update: string;
  video_fragments: number | null;
  audio_fragments: number | null;
  total_size: string | null;
  video_quality: string | null;
  output_file: string | null;
};

export type State =
  | { Waiting: string }
  | 'Recording'
  | 'Muxing'
  | 'Finished'
  | 'Idle'
  | 'Ended'
  | 'AlreadyProcessed'
  | 'Interrupted';

export const stateString = (state: State) => {
  if (typeof state === 'object' && 'Waiting' in state)
    return 'Waiting (' + state.Waiting + ')';
  else if (state === 'AlreadyProcessed') return 'Already Processed';
  else return state;
};
export const stateKey = (state: State) =>
  typeof state === 'object' ? Object.keys(state)[0] : state;

export const useQueryTasks = () =>
  useQuery(
    ['tasks'],
    () =>
      fetch('/api/tasks')
        .then((res) => res.json())
        .then((res) => res as TaskWithStatus[]),
    {
      refetchInterval: 1000,
      keepPreviousData: true,
    }
  );
