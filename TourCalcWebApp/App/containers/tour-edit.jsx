import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'

import Dialog from '@material-ui/core/Dialog';
import DialogTitle from '@material-ui/core/DialogTitle';
import DialogContent from '@material-ui/core/DialogContent';
import DialogActions from '@material-ui/core/DialogActions';

export default class PersonForm extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            dialogOpen: props.open
        }
        //alert('open: ' + props.open)
    }
    name = ""
    weight = 1
    render() {
        return (
            <span>
                <button onClick={() => this.setState({ dialogOpen: true })}>{this.props.buttonText}</button>
            <Dialog aria-labelledby="customized-dialog-title" open={this.state.dialogOpen}>
                <DialogTitle id="customized-dialog-title">Add Person</DialogTitle>
                    <DialogContent>
                        <form onSubmit={(event) => {
                            event.preventDefault();
                            //alert('sending')
                            AppState.addPerson(this.props.app, this.props.tourid, { name: this.name, weight: (this.weight * 100) })
                                .then(AppState.loadTour(this.props.app, this.props.tourid))
                            }}>
                            <p>name:</p>
                            <input
                                type='text'
                                onChange={(e) => this.name = event.target.value}
                            />
                            <p>weight:</p>
                            <input
                                type='number'
                                onChange={(e) => this.weight = event.target.value}
                                defaultValue={this.weight}
                            />
                        </form>
                </DialogContent>
                <DialogActions>
                        <button color="primary" onClick={() => {
                            AppState.addPerson(this.props.app, this.props.tourid, { name: this.name, weight: (this.weight * 100) })
                                .then(this.setState({ dialogOpen: false }))
                                .then(() => { AppState.loadTour(this.props.app, this.props.tourid) })

                        }}>{this.props.actionButtonText}</button>
                </DialogActions>
            </Dialog>
            </span>
        )
    }
}