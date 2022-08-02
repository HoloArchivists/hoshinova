import {
  Anchor,
  AspectRatio,
  Badge,
  Card,
  Container,
  Image,
  MediaQuery,
  Stack,
  Table,
  Text,
  Title,
} from '@mantine/core';
import React from 'react';
import {
  State,
  stateKey,
  stateString,
  TaskWithStatus,
  useQueryTasks,
} from '../api/tasks';
import { SuspenseLoader } from '../shared/SuspenseLoader';

const SleepingPanda = React.lazy(() => import('../lotties/SleepingPanda'));

const TaskStateBadge = ({ state }: { state: State }) => (
  <Badge
    color={
      state === 'Recording'
        ? 'green'
        : state === 'Finished'
        ? 'blue'
        : state === 'Muxing'
        ? 'yellow'
        : state === 'Idle' || state === 'AlreadyProcessed' || state === 'Ended'
        ? 'gray'
        : state === 'Interrupted'
        ? 'red'
        : 'violet'
    }
    variant="filled"
  >
    {stateString(state)}
  </Badge>
);

const rowElements = ({ task, status }: TaskWithStatus) => [
  <Image width={160} height={90} radius="md" src={task.video_picture} />,
  <>
    <Anchor
      style={{ display: 'block' }}
      href={'https://www.youtube.com/watch?v=' + task.video_id}
    >
      {task.title}
    </Anchor>
    <Anchor
      color="dimmed"
      style={{ display: 'block' }}
      href={'https://www.youtube.com/channel/' + task.channel_id}
    >
      {task.channel_name}
    </Anchor>
  </>,
  <TaskStateBadge state={status.state} />,
  <>
    {status.total_size === null ? (
      'None'
    ) : (
      <>
        V: {status.video_fragments || '?'} / A: {status.audio_fragments || '?'}{' '}
        / DL: {status.total_size || '?'}
      </>
    )}
  </>,
];

const TasksPage = () => {
  const qTasks = useQueryTasks();

  const stateSort = [
    'Recording',
    'Muxing',
    'Waiting',
    'Finished',
    'Idle',
    'Ended',
    'AlreadyProcessed',
    'Interrupted',
  ];
  const tasks = !qTasks.data
    ? []
    : qTasks.data.sort(
        (a, b) =>
          stateSort.indexOf(stateKey(a.status.state)) -
          stateSort.indexOf(stateKey(b.status.state))
      );

  if (qTasks.isLoading && !qTasks.data) return <SuspenseLoader />;

  if (tasks.length < 1)
    return (
      <Container size="xs">
        <AspectRatio ratio={1}>
          <React.Suspense fallback={<div />}>
            <SleepingPanda />
          </React.Suspense>
        </AspectRatio>
        <Title>Crickets...</Title>
        <Text>
          There's nothing here yet. Maybe add some more channels to spice things
          up!
        </Text>
      </Container>
    );

  return (
    <>
      <MediaQuery smallerThan="xs" styles={{ display: 'none' }}>
        <Table>
          <thead>
            <tr>
              <th>Thumbnail</th>
              <th>Video</th>
              <th>Status</th>
              <th>Progress</th>
            </tr>
          </thead>
          <tbody>
            {tasks.map(({ task, status }) => (
              <tr key={task.video_id}>
                {rowElements({ task, status }).map((row, idx) => (
                  <td key={idx}>{row}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </Table>
      </MediaQuery>
      <MediaQuery largerThan="xs" styles={{ display: 'none' }}>
        <Stack spacing="md">
          {tasks.map(({ task, status }) => {
            const [_, title, state, progres] = rowElements({ task, status });
            return (
              <Card key={task.video_id}>
                <Card.Section>
                  <AspectRatio ratio={16 / 9}>
                    <Image fit="cover" width="100%" src={task.video_picture} />
                  </AspectRatio>
                </Card.Section>
                <Stack my="lg" spacing="md">
                  <div>{title}</div>
                  {state}
                  <div>
                    <Text weight="bold">Progress</Text>
                    {progres}
                  </div>
                </Stack>
              </Card>
            );
          })}
        </Stack>
      </MediaQuery>
    </>
  );
};

export default TasksPage;
