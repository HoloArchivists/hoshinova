import { Container, Loader, Tabs } from '@mantine/core';
import React, { Suspense } from 'react';
import ConfigPage from './pages/ConfigPage';

const TasksPage = React.lazy(() => import('./pages/TasksPage'));

const LoadingContainer = () => (
  <Container
    p="xl"
    sx={() => ({
      display: 'flex',
      justifyContent: 'center',
    })}
  >
    <Loader />
  </Container>
);

function App() {
  return (
    <>
      <Tabs defaultValue="tasks">
        <Tabs.List>
          <Tabs.Tab value="tasks">Tasks</Tabs.Tab>
          <Tabs.Tab value="config">Configuration</Tabs.Tab>
        </Tabs.List>

        <Tabs.Panel value="tasks">
          <Suspense fallback={<LoadingContainer />}>
            <TasksPage />
          </Suspense>
        </Tabs.Panel>
        <Tabs.Panel value="config">
          <Suspense fallback={<LoadingContainer />}>
            <ConfigPage />
          </Suspense>
        </Tabs.Panel>
      </Tabs>
    </>
  );
}

export default App;
