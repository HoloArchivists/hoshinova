import { Tabs } from '@mantine/core';
import React, { Suspense } from 'react';
import { SuspenseLoader } from './shared/SuspenseLoader';

const TasksPage = React.lazy(() => import('./pages/TasksPage'));
const ConfigPage = React.lazy(() => import('./pages/ConfigPage'));

function App() {
  return (
    <>
      <Tabs defaultValue="tasks">
        <Tabs.List>
          <Tabs.Tab value="tasks">Tasks</Tabs.Tab>
          <Tabs.Tab value="config">Configuration</Tabs.Tab>
        </Tabs.List>

        <Tabs.Panel value="tasks">
          <Suspense fallback={<SuspenseLoader />}>
            <TasksPage />
          </Suspense>
        </Tabs.Panel>
        <Tabs.Panel value="config">
          <Suspense fallback={<SuspenseLoader />}>
            <ConfigPage />
          </Suspense>
        </Tabs.Panel>
      </Tabs>
    </>
  );
}

export default App;
