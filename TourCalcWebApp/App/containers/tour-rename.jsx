import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'

import Dialog from '@material-ui/core/Dialog';
import DialogTitle from '@material-ui/core/DialogTitle';
import DialogContent from '@material-ui/core/DialogContent';
import DialogActions from '@material-ui/core/DialogActions';

export default class TourNameEdit extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            dialogOpen: props.open
        }
        this.name = props.name
    }

    name = ""

    render() {
        return (
            <span>
                <button onClick={() => this.setState({ dialogOpen: true })}>
                    {this.props.buttonText}
                </button>
            <Dialog aria-labelledby="customized-dialog-title" open={this.state.dialogOpen}>
                <DialogTitle id="customized-dialog-title">Change Tour Name</DialogTitle>
                    <DialogContent>
                        <form onSubmit={(event) => {
                            event.preventDefault();
                            }}>
                            <p>name:</p>
                            <input
                                type='text'
                                onChange={(e) => this.name = event.target.value}
                                defaultValue={this.name}
                            />
                        </form>
                </DialogContent>
                <DialogActions>
                        <button color="primary" onClick={() => {
                            AppState.changeTourName(this.props.app, this.props.tourid, this.name)
                                //.then(res => { alert('res: ' + res) }, error => { alert('error: ' + error) })
                                .then(() => this.setState({ dialogOpen: false }))
                                .then(() => { AppState.loadTours(this.props.app, this.props.tourid) })

                        }}>{this.props.actionButtonText}</button>
                        <button onClick={() => { this.setState({ dialogOpen: false }) }}>Cancel</button>
                </DialogActions>
            </Dialog>
            </span>
        )
    }
}
