import React from 'react';
import {
  Button,
  Container,
  Group,
  Switch,
  Text,
  Textarea,
} from '@mantine/core';
import { showNotification } from '@mantine/notifications';
import {
  useMutateConfigTOML,
  useMutateReloadConfig,
  useQueryConfigTOML,
} from '../api/config';

const ConfigPage = () => {
  const qConfig = useQueryConfigTOML();
  const mReload = useMutateReloadConfig();
  const mUpdate = useMutateConfigTOML();
  const [isEditable, setIsEditable] = React.useState(false);
  const [textContent, setTextContent] = React.useState('');

  // Update the textarea with the latest configuration if it's not being edited
  React.useEffect(() => {
    if (!qConfig.data) return;
    if (textContent === '' || !isEditable) setTextContent(qConfig.data);
  }, [qConfig, isEditable]);

  const saveReload = () => {
    if (!isEditable)
      // Reload configuration
      mReload.mutate(undefined, {
        onSuccess() {
          showNotification({
            message: 'Configuration reloaded',
            color: 'green',
          });
        },
      });
    // Update configuration with the textarea content
    else
      mUpdate.mutate(textContent, {
        onSuccess() {
          showNotification({
            message: 'Configuration updated',
            color: 'green',
          });
        },
        async onError(err) {
          let message = '';
          if (err instanceof Response) message = await err.text();
          showNotification({
            title: 'Error updating configuration',
            message,
            color: 'red',
          });
          console.error(err);
        },
      });
  };

  return (
    <Container fluid py="md">
      <Group pb="md" position="apart">
        <Group>
          <Button
            onClick={saveReload}
            disabled={mReload.isLoading || mUpdate.isLoading}
          >
            {mUpdate.isLoading
              ? 'Saving...'
              : mReload.isLoading
              ? 'Reloading...'
              : isEditable
              ? 'Save and apply'
              : 'Reload configuration'}
          </Button>
          {isEditable ? null : (
            <Text size="sm">
              TOML configuration may differ from the one used by the server.
              Reload the configuration to apply any changes.
            </Text>
          )}
        </Group>
        <Switch
          label="Enable editing"
          checked={isEditable}
          onChange={(evt) => setIsEditable(evt.currentTarget.checked)}
        />
      </Group>
      <Textarea
        value={textContent}
        onChange={(evt) => setTextContent(evt.currentTarget.value)}
        autosize
        styles={{ input: { fontFamily: 'monospace' } }}
        disabled={!isEditable || mReload.isLoading || mUpdate.isLoading}
      />
    </Container>
  );
};

export default ConfigPage;
